#!/usr/bin/env bash
# ==============================================================================
# Syncify — Pre-Redownload Baseline Snapshot Tool (S180)
# ==============================================================================
# Captures an immutable, read-only baseline snapshot of the current library:
# 1. SQLite DB snapshot (tracks, downloads, download_queue) -> /tmp/tag_audit/baseline_*.json
# 2. Complete filesystem tag dump (FLAC VorbisComments & MP4 atoms) -> /tmp/tag_audit/baseline_tags.jsonl
# ==============================================================================
set -euo pipefail

AUDIT_DIR="/tmp/tag_audit"
mkdir -p "$AUDIT_DIR"

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
DB_PATH="${HOME}/.local/share/com.syncify.app/syncify.db"
MUSIC_DIR="${HOME}/Music/Syncify"
DB_SNAPSHOT="${AUDIT_DIR}/baseline_${TIMESTAMP}.json"
TAGS_SNAPSHOT="${AUDIT_DIR}/baseline_tags.jsonl"
LATEST_DB="${AUDIT_DIR}/baseline_db.json"

echo "=== S180: Generating Pre-Redownload Baseline Snapshot [${TIMESTAMP}] ==="

# 1. Snapshot SQLite Database state
if [ -f "$DB_PATH" ]; then
    echo "[1/2] Snapshotting Syncify SQLite Database..."
    python3 -c "
import sqlite3, json, os

db = os.path.expanduser('$DB_PATH')
conn = sqlite3.connect(db)
conn.row_factory = sqlite3.Row
cur = conn.cursor()

data = {
    'timestamp': '$TIMESTAMP',
    'db_path': db,
    'tracks_count': 0,
    'downloads_count': 0,
    'queue_count': 0,
    'downloads': [],
    'queue_summary': {}
}

cur.execute('SELECT COUNT(*) FROM tracks;')
data['tracks_count'] = cur.fetchone()[0]

cur.execute('SELECT COUNT(*) FROM downloads;')
data['downloads_count'] = cur.fetchone()[0]

cur.execute('SELECT COUNT(*) FROM download_queue;')
data['queue_count'] = cur.fetchone()[0]

cur.execute('SELECT status, skip_reason, COUNT(*) as cnt FROM download_queue GROUP BY status, skip_reason;')
for row in cur.fetchall():
    key = f\"{row['status']}:{row['skip_reason'] or 'none'}\"
    data['queue_summary'][key] = row['cnt']

cur.execute('''
    SELECT d.id, d.track_id, d.file_path, d.file_format, d.file_size_bytes,
           d.effective_service, d.effective_quality, d.downloaded_at,
           t.title, t.isrc, t.genre, t.subgenre
    FROM downloads d
    LEFT JOIN tracks t ON t.id = d.track_id
''')
for row in cur.fetchall():
    data['downloads'].append(dict(row))

with open('$DB_SNAPSHOT', 'w', encoding='utf-8') as f:
    json.dump(data, f, indent=2)

with open('$LATEST_DB', 'w', encoding='utf-8') as f:
    json.dump(data, f, indent=2)

print(f'  [OK] Saved DB snapshot: {data[\"tracks_count\"]} tracks, {data[\"downloads_count\"]} downloads -> $DB_SNAPSHOT')
"
else
    echo "  [WARN] Database not found at $DB_PATH"
fi

# 2. Snapshot Physical Tags across all files
echo "[2/2] Scanning physical audio files under $MUSIC_DIR..."
python3 -c "
import os, json, subprocess

music_dir = os.path.expanduser('$MUSIC_DIR')
output_file = '$TAGS_SNAPSHOT'

flac_count = 0
mp4_count = 0
records = []

for root, dirs, files in os.walk(music_dir):
    for f in sorted(files):
        ext = os.path.splitext(f)[1].lower()
        if ext not in ('.flac', '.m4a', '.mp4'):
            continue
        
        full_path = os.path.join(root, f)
        rel_path = os.path.relpath(full_path, music_dir)
        size_bytes = os.path.getsize(full_path)
        mtime = os.path.getmtime(full_path)

        record = {
            'file_path': full_path,
            'rel_path': rel_path,
            'format': ext[1:].upper(),
            'size_bytes': size_bytes,
            'mtime': mtime,
            'tags': {}
        }

        if ext == '.flac':
            flac_count += 1
            try:
                proc = subprocess.run(
                    ['metaflac', '--export-tags-to=-', full_path],
                    capture_output=True, text=True, check=True
                )
                for line in proc.stdout.splitlines():
                    if '=' in line:
                        k, v = line.split('=', 1)
                        k_norm = k.strip().upper()
                        v_str = v.strip()
                        if k_norm not in record['tags']:
                            record['tags'][k_norm] = []
                        record['tags'][k_norm].append(v_str)
            except Exception as e:
                record['error'] = str(e)
        elif ext in ('.m4a', '.mp4'):
            mp4_count += 1
            try:
                import mutagen.mp4
                mp4 = mutagen.mp4.MP4(full_path)
                for k, v in mp4.tags.items():
                    key_str = str(k)
                    if isinstance(v, list):
                        record['tags'][key_str] = [str(x) for x in v]
                    else:
                        record['tags'][key_str] = [str(v)]
            except Exception:
                pass

        records.append(record)

with open(output_file, 'w', encoding='utf-8') as f:
    for r in records:
        f.write(json.dumps(r, ensure_ascii=False) + '\n')

print(f'  [OK] Processed {len(records)} files ({flac_count} FLACs, {mp4_count} MP4s) -> {output_file}')
"

echo ""
echo "=== Baseline Snapshot Successfully Created ==="
echo "  Database Snapshot: $DB_SNAPSHOT"
echo "  Physical Tags:     $TAGS_SNAPSHOT"
