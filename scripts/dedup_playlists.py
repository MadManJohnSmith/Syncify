#!/usr/bin/env python3
"""
Syncify Playlist Deduplication Script (Task F2.4 / Mitigates A3)
==============================================================
Identifies and safely deduplicates redundant playlists within the same account
having identical normalized names (LOWER(TRIM(name))).

Mitigation Strategy:
1. Group playlists by `(account_id, LOWER(TRIM(name)))` with count > 1.
2. For each duplicate group:
   - Select primary (winner) playlist with most tracks (tie-break: newest updated_at, highest id).
   - Evaluate candidates:
     * Jaccard similarity (|A ∩ B| / |A ∪ B|)
     * Track containment (|A ∩ B| / |A|)
     * Exact subset or empty playlist detection.
   - Reassign any tracks present in loser but not in winner (appending sequentially).
   - Unify `playlist_sources` linking external services to winner.
   - Remove loser rows from `playlist_tracks` and `playlist_sources`.
   - Delete redundant loser playlists from `playlists`.
   - Recalculate winner's `track_count` and update `updated_at`.
3. Wrap everything in a single atomic transaction:
   `PRAGMA foreign_keys = ON; BEGIN TRANSACTION; ... COMMIT;`
4. Verify `PRAGMA foreign_key_check`.
"""

import argparse
import os
import sqlite3
import sys
import time
from typing import Dict, List, Set, Tuple, Any


def escape_sql_str(val: Any) -> str:
    if val is None:
        return "NULL"
    return "'" + str(val).replace("'", "''") + "'"


def analyze_and_deduplicate(
    db_path: str,
    sql_out_path: str,
    dry_run: bool = False,
    min_jaccard: float = 0.70,
    min_containment: float = 0.90,
) -> Dict[str, Any]:
    if not os.path.exists(db_path):
        raise FileNotFoundError(f"Database file not found: {db_path}")

    print(f"Opening database: {db_path}")
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()

    # Enable WAL and foreign keys
    cur.execute("PRAGMA journal_mode = WAL;")
    cur.execute("PRAGMA foreign_keys = ON;")

    # Initial metrics
    initial_total_pls = cur.execute("SELECT count(*) FROM playlists").fetchone()[0]
    initial_total_pl_tracks = cur.execute("SELECT count(*) FROM playlist_tracks").fetchone()[0]
    initial_total_pl_sources = cur.execute("SELECT count(*) FROM playlist_sources").fetchone()[0]

    # Find duplicate groups
    dup_groups_query = """
        SELECT p.account_id, LOWER(TRIM(p.name)) as norm_name, count(*) as cnt
        FROM playlists p
        GROUP BY p.account_id, LOWER(TRIM(p.name))
        HAVING cnt > 1
        ORDER BY p.account_id, norm_name
    """
    dup_groups = cur.execute(dup_groups_query).fetchall()

    print(f"Identified {len(dup_groups)} duplicate groups ({sum(g[2] for g in dup_groups)} total candidate playlists).")

    sql_lines: List[str] = []
    sql_lines.append("-- =====================================================================")
    sql_lines.append("-- Syncify Playlist Deduplication SQL Script (Task F2.4 / Mitigates A3)")
    sql_lines.append(f"-- Generated: {time.strftime('%Y-%m-%d %H:%M:%S UTC', time.gmtime())}")
    sql_lines.append(f"-- Database: {db_path}")
    sql_lines.append(f"-- Duplicate groups analyzed: {len(dup_groups)}")
    sql_lines.append("-- =====================================================================\n")
    sql_lines.append("PRAGMA foreign_keys = ON;")
    sql_lines.append("BEGIN TRANSACTION;\n")

    groups_analyzed = len(dup_groups)
    playlists_merged = 0
    extra_tracks_appended = 0
    sources_reassigned = 0
    group_reports: List[Dict[str, Any]] = []

    for group_idx, (acc_id, norm_name, cnt) in enumerate(dup_groups, 1):
        # Fetch all playlists in this group
        pls = cur.execute("""
            SELECT id, account_id, service_playlist_id, name, description, is_public,
                   track_count, last_synced, created_at, updated_at, owner_name, owner_id,
                   is_collaborative, image_url
            FROM playlists
            WHERE account_id = ? AND LOWER(TRIM(name)) = ?
            ORDER BY id ASC
        """, (acc_id, norm_name)).fetchall()

        pl_candidates: List[Dict[str, Any]] = []
        for pl in pls:
            pid = pl[0]
            tracks = cur.execute("""
                SELECT position, track_id, added_at
                FROM playlist_tracks
                WHERE playlist_id = ?
                ORDER BY position ASC
            """, (pid,)).fetchall()

            sources = cur.execute("""
                SELECT id, account_id, service_id, service_playlist_id, synced_at
                FROM playlist_sources
                WHERE playlist_id = ?
            """, (pid,)).fetchall()

            pl_candidates.append({
                "id": pid,
                "account_id": pl[1],
                "service_playlist_id": pl[2],
                "name": pl[3],
                "description": pl[4],
                "is_public": pl[5],
                "track_count": pl[6],
                "last_synced": pl[7],
                "created_at": pl[8] or "",
                "updated_at": pl[9] or "",
                "owner_name": pl[10],
                "owner_id": pl[11],
                "is_collaborative": pl[12],
                "image_url": pl[13],
                "tracks": tracks,
                "track_ids": [t[1] for t in tracks],
                "track_set": set(t[1] for t in tracks),
                "sources": sources,
            })

        # Select primary (winner):
        # 1. Most actual tracks in playlist_tracks
        # 2. Newest updated_at / last_synced / created_at
        # 3. Highest ID
        pl_candidates.sort(
            key=lambda x: (len(x["track_set"]), x["updated_at"], x["last_synced"] or "", x["created_at"], x["id"]),
            reverse=True
        )

        winner = pl_candidates[0]
        winner_id = winner["id"]
        winner_track_set = set(winner["track_set"])
        winner_max_pos = max([t[0] for t in winner["tracks"]], default=0)

        sql_lines.append(f"-- ---------------------------------------------------------------------")
        sql_lines.append(f"-- Group {group_idx}/{groups_analyzed}: \"{norm_name}\" (Account {acc_id}, {cnt} playlists)")
        sql_lines.append(f"-- Primary Winner: ID {winner_id} ('{winner['name']}'), initial tracks: {len(winner_track_set)}")
        sql_lines.append(f"-- ---------------------------------------------------------------------")

        group_report = {
            "group_idx": group_idx,
            "account_id": acc_id,
            "name": norm_name,
            "winner_id": winner_id,
            "winner_initial_tracks": len(winner_track_set),
            "merged_losers": [],
        }

        for cand in pl_candidates[1:]:
            loser_id = cand["id"]
            cand_track_set = cand["track_set"]

            # Calculate metrics
            union = winner_track_set | cand_track_set
            inter = winner_track_set & cand_track_set
            jaccard = len(inter) / len(union) if union else 1.0
            containment = len(inter) / len(cand_track_set) if cand_track_set else 1.0
            is_subset = cand_track_set.issubset(winner_track_set)
            is_empty = (len(cand_track_set) == 0)

            # Verification check
            eligible = (jaccard >= min_jaccard) or (containment >= min_containment) or is_subset or is_empty
            if not eligible:
                raise ValueError(
                    f"Candidate {loser_id} in group '{norm_name}' failed eligibility check: "
                    f"Jaccard={jaccard:.4f} (< {min_jaccard}), Containment={containment:.4f} (< {min_containment})"
                )

            # Find extra tracks in loser not in winner
            extra_tracks = [t for t in cand["tracks"] if t[1] not in winner_track_set]

            sql_lines.append(f"-- Merging Loser ID {loser_id} ('{cand['name']}'): Jaccard={jaccard:.3f}, Containment={containment:.3f}, Extra Tracks={len(extra_tracks)}")

            # 1. Append extra tracks to winner
            for pos, tid, added_at in extra_tracks:
                winner_max_pos += 1
                winner_track_set.add(tid)
                extra_tracks_appended += 1
                added_at_val = escape_sql_str(added_at)
                sql_lines.append(
                    f"INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) "
                    f"VALUES ({winner_id}, {tid}, {winner_max_pos}, {added_at_val});"
                )

            # 2. Reassign playlist_sources
            # Remove any conflicting source rows first (same account_id & service_playlist_id)
            sql_lines.append(
                f"DELETE FROM playlist_sources WHERE playlist_id = {loser_id} AND (account_id, service_playlist_id) IN "
                f"(SELECT account_id, service_playlist_id FROM playlist_sources WHERE playlist_id = {winner_id});"
            )
            sql_lines.append(
                f"UPDATE playlist_sources SET playlist_id = {winner_id} WHERE playlist_id = {loser_id};"
            )
            sources_reassigned += len(cand["sources"])

            # 3. Delete loser playlist_tracks
            sql_lines.append(f"DELETE FROM playlist_tracks WHERE playlist_id = {loser_id};")

            # 4. Delete loser playlist
            sql_lines.append(f"DELETE FROM playlists WHERE id = {loser_id};")

            playlists_merged += 1
            group_report["merged_losers"].append({
                "loser_id": loser_id,
                "loser_name": cand["name"],
                "jaccard": jaccard,
                "containment": containment,
                "extra_tracks": len(extra_tracks),
            })

        # Update winner metadata: description (if richer), track_count, updated_at
        sql_lines.append(
            f"UPDATE playlists SET "
            f"track_count = (SELECT count(*) FROM playlist_tracks WHERE playlist_id = {winner_id}), "
            f"updated_at = CURRENT_TIMESTAMP "
            f"WHERE id = {winner_id};\n"
        )

        group_report["winner_final_tracks"] = len(winner_track_set)
        group_reports.append(group_report)

    sql_lines.append("COMMIT;\n")
    sql_lines.append("PRAGMA foreign_key_check;\n")

    full_sql = "\n".join(sql_lines)

    # Write SQL script
    os.makedirs(os.path.dirname(os.path.abspath(sql_out_path)), exist_ok=True)
    with open(sql_out_path, "w", encoding="utf-8") as f:
        f.write(full_sql)
    print(f"Generated SQL script written to: {sql_out_path} ({len(sql_lines)} lines)")

    if dry_run:
        print("Dry run requested. Simulating script execution in a transaction with ROLLBACK...")
        cur.execute("BEGIN TRANSACTION;")
        # Execute each statement
        for statement in full_sql.split(";"):
            stmt = statement.strip()
            if stmt and not stmt.startswith("--") and stmt != "BEGIN TRANSACTION" and stmt != "COMMIT" and not stmt.startswith("PRAGMA"):
                cur.execute(stmt)
        remaining_dup_groups = cur.execute(dup_groups_query).fetchall()
        final_total_pls = cur.execute("SELECT count(*) FROM playlists").fetchone()[0]
        final_total_pl_tracks = cur.execute("SELECT count(*) FROM playlist_tracks").fetchone()[0]
        final_total_pl_sources = cur.execute("SELECT count(*) FROM playlist_sources").fetchone()[0]
        fk_violations = cur.execute("PRAGMA foreign_key_check;").fetchall()
        cur.execute("ROLLBACK;")
        print(f"Dry run complete. Simulated post-dedup: remaining dup groups = {len(remaining_dup_groups)}, FK violations = {len(fk_violations)}")
    else:
        print("Applying transaction to database...")
        # Execute the SQL script directly
        cur.executescript(full_sql)
        conn.commit()
        print("Transaction successfully committed.")

        # Post-execution verification
        cur.execute("PRAGMA foreign_keys = ON;")
        fk_violations = cur.execute("PRAGMA foreign_key_check;").fetchall()
        remaining_dup_groups = cur.execute(dup_groups_query).fetchall()
        final_total_pls = cur.execute("SELECT count(*) FROM playlists").fetchone()[0]
        final_total_pl_tracks = cur.execute("SELECT count(*) FROM playlist_tracks").fetchone()[0]
        final_total_pl_sources = cur.execute("SELECT count(*) FROM playlist_sources").fetchone()[0]

    conn.close()

    result = {
        "groups_analyzed": groups_analyzed,
        "playlists_merged": playlists_merged,
        "initial_total_playlists": initial_total_pls,
        "final_total_playlists": final_total_pls,
        "expected_final_playlists": initial_total_pls - playlists_merged,
        "extra_tracks_appended": extra_tracks_appended,
        "initial_total_pl_sources": initial_total_pl_sources,
        "final_total_pl_sources": final_total_pl_sources,
        "remaining_duplicate_groups": len(remaining_dup_groups),
        "foreign_key_violations": len(fk_violations),
        "fk_violations_detail": fk_violations,
        "group_reports": group_reports,
    }

    return result


def main():
    parser = argparse.ArgumentParser(description="Syncify Playlist Deduplication Tool (Task F2.4)")
    parser.add_argument("--db", default="syncify_backup_pre_repair.db", help="Path to SQLite database file")
    parser.add_argument("--sql-out", default="scripts/dedup_playlists.sql", help="Path to output SQL script")
    parser.add_argument("--dry-run", action="store_true", help="Simulate without committing changes")
    parser.add_argument("--min-jaccard", type=float, default=0.70, help="Minimum Jaccard similarity threshold")
    parser.add_argument("--min-containment", type=float, default=0.90, help="Minimum track containment threshold")

    args = parser.parse_args()

    result = analyze_and_deduplicate(
        db_path=args.db,
        sql_out_path=args.sql_out,
        dry_run=args.dry_run,
        min_jaccard=args.min_jaccard,
        min_containment=args.min_containment,
    )

    print("\n" + "=" * 60)
    print("PLAYLIST DEDUPLICATION AUDIT SUMMARY (F2.4 / A3)")
    print("=" * 60)
    print(f"Groups Analyzed:                 {result['groups_analyzed']}")
    print(f"Playlists Merged / Removed:      {result['playlists_merged']}")
    print(f"Initial Total Playlists:         {result['initial_total_playlists']}")
    print(f"Final Total Playlists:           {result['final_total_playlists']} (Expected: {result['expected_final_playlists']})")
    print(f"Extra Tracks Reassigned:         {result['extra_tracks_appended']}")
    print(f"Initial Playlist Sources:        {result['initial_total_pl_sources']}")
    print(f"Final Playlist Sources:          {result['final_total_pl_sources']}")
    print(f"Remaining Duplicate Groups:      {result['remaining_duplicate_groups']}")
    print(f"Foreign Key Violations:          {result['foreign_key_violations']}")
    print("=" * 60)


if __name__ == "__main__":
    main()
