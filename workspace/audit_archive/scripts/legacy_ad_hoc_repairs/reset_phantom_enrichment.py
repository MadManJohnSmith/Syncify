#!/usr/bin/env python3
"""
F4.7 / M11: Reset enrichment_status to 'pending' for tracks where bpm IS NULL.
"""

import argparse
import sqlite3
import sys
from pathlib import Path

def reset_phantom_enrichment(db_path: str):
    print(f"[*] Opening database: {db_path}")
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    cursor.execute("PRAGMA foreign_keys = ON;")

    # Metrics before
    cursor.execute("SELECT count(*) FROM tracks;")
    total_tracks = cursor.fetchone()[0]

    cursor.execute("SELECT count(*) FROM tracks WHERE bpm IS NULL;")
    bpm_null_count = cursor.fetchone()[0]

    cursor.execute("SELECT count(*) FROM tracks WHERE bpm IS NOT NULL;")
    bpm_not_null_count = cursor.fetchone()[0]

    cursor.execute("SELECT enrichment_status, count(*) FROM tracks WHERE bpm IS NULL GROUP BY enrichment_status;")
    before_status = cursor.fetchall()

    print(f"[*] Total tracks: {total_tracks}")
    print(f"[*] Tracks with bpm IS NULL: {bpm_null_count}")
    print(f"[*] Tracks with bpm IS NOT NULL: {bpm_not_null_count}")
    print(f"[*] Status before update for bpm IS NULL: {before_status}")

    # Transactional update
    cursor.execute("BEGIN TRANSACTION;")
    cursor.execute("UPDATE tracks SET enrichment_status = 'pending' WHERE bpm IS NULL;")
    conn.commit()

    # Metrics after
    cursor.execute("SELECT enrichment_status, count(*) FROM tracks WHERE bpm IS NULL GROUP BY enrichment_status;")
    after_status = cursor.fetchall()

    cursor.execute("SELECT count(*) FROM tracks WHERE bpm IS NULL AND enrichment_status != 'pending';")
    non_pending_null_bpm = cursor.fetchone()[0]

    cursor.execute("PRAGMA foreign_key_check;")
    fk_violations = cursor.fetchall()

    conn.close()

    print(f"[*] Status after update for bpm IS NULL: {after_status}")
    print(f"[*] Tracks with bpm IS NULL and status != 'pending': {non_pending_null_bpm}")
    print(f"[*] Foreign key check violations: {len(fk_violations)}")

    if non_pending_null_bpm != 0 or len(fk_violations) != 0:
        print("[!] Error in verification checks", file=sys.stderr)
        sys.exit(1)
    print("[*] Successfully reset enrichment_status to pending for tracks with bpm IS NULL")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Reset phantom enrichment for tracks with bpm IS NULL")
    parser.add_argument("--db", default="syncify_backup_pre_repair.db", help="Path to database file")
    args = parser.parse_args()
    reset_phantom_enrichment(args.db)
