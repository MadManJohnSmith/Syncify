#!/usr/bin/env python3
"""
S150/S153 Audit Helper: Parse and pretty-print NDJSON live-network audit telemetry.
Archived script for inspecting historical pilot run logs.
"""

import argparse
import json
import os
import sqlite3
import sys

if sys.platform == 'win32':
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description="Parse and display NDJSON audit logs from S150 live network pilot")
    parser.add_argument("--ndjson-path", required=True, help="Path to the .ndjson log file")
    parser.add_argument("--db-path", help="Optional path to SQLite database to display downloads schema")
    args = parser.parse_args()

    if args.db_path and os.path.exists(args.db_path):
        conn = sqlite3.connect(f"file:{os.path.abspath(args.db_path)}?mode=ro", uri=True)
        print("=== DOWNLOADS SCHEMA ===")
        schema = conn.execute("SELECT sql FROM sqlite_master WHERE name='downloads'").fetchone()
        if schema:
            print(schema[0])
        conn.close()
        print()

    if not os.path.exists(args.ndjson_path):
        print(f"Error: NDJSON file not found at {args.ndjson_path}", file=sys.stderr)
        sys.exit(1)

    print(f"=== PARSING AUDIT LOG: {args.ndjson_path} ===\n")
    with open(args.ndjson_path, "r", encoding="utf-8") as f:
        for i, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            d = json.loads(line)
            print(f"Track {i:02d}: ID {d.get('track_id')} | '{d.get('title')}' by {d.get('artist')}")
            print(f"  Origin: {d.get('origin_service')} | Effective Provider: {d.get('effective_provider')} | Preflight: {d.get('preflight_decision')}")
            print(f"  Source Track ID: {d.get('service_track_id')} | URL Class: {d.get('url_class')}")
            file_size = d.get('file_size_bytes', 0)
            print(f"  Status: {d.get('status')} | Size: {file_size:,} bytes ({file_size / 1048576:.2f} MB)")
            print(f"  Codec: {d.get('ffprobe_codec')} | SR: {d.get('ffprobe_sample_rate')} Hz | Bits: {d.get('ffprobe_bit_depth')} | Dur: {d.get('ffprobe_duration_sec', 0.0):.2f} s")
            print(f"  Transfer Time: {d.get('transfer_duration_ms')} ms | Speed: {d.get('throughput_mibps', 0.0):.2f} MiB/s")
            print(f"  SHA256: {d.get('sha256')}")
            print(f"  File: {d.get('file_path')}")
            print(f"  Tags Valid: {d.get('tagging_verified')} | Magic: {d.get('magic_bytes_valid')} | Staging Clean: {d.get('staging_cleaned')}")
            print()


if __name__ == "__main__":
    main()
