import sqlite3, json, os, sys
sys.stdout.reconfigure(encoding='utf-8')

conn = sqlite3.connect('C:/Users/tardis/AppData/Local/com.syncify.app/syncify.db')
print('=== DOWNLOADS SCHEMA ===')
print(conn.execute("SELECT sql FROM sqlite_master WHERE name='downloads'").fetchone()[0])

ndjson_path = r'F:\Syncify-Control-1\s150_live_network_audit.ndjson'
if os.path.exists(ndjson_path):
    with open(ndjson_path, 'r', encoding='utf-8') as f:
        for i, line in enumerate(f, 1):
            d = json.loads(line.strip())
            print(f"Track {i:02d}: ID {d.get('track_id')} | '{d.get('title')}' by {d.get('artist')}")
            print(f"  Origin: {d.get('origin_service')} | Effective Provider: {d.get('effective_provider')} | Preflight: {d.get('preflight_decision')}")
            print(f"  Source Track ID: {d.get('service_track_id')} | URL Class: {d.get('url_class')}")
            print(f"  Status: {d.get('status')} | Size: {d.get('file_size_bytes'):,} bytes ({d.get('file_size_bytes')/1048576:.2f} MB)")
            print(f"  Codec: {d.get('ffprobe_codec')} | SR: {d.get('ffprobe_sample_rate')} Hz | Bits: {d.get('ffprobe_bit_depth')} | Dur: {d.get('ffprobe_duration_sec'):.2f} s")
            print(f"  Transfer Time: {d.get('transfer_duration_ms')} ms | Speed: {d.get('throughput_mibps'):.2f} MiB/s")
            print(f"  SHA256: {d.get('sha256')}")
            print(f"  File: {d.get('file_path')}")
            print(f"  Tags Valid: {d.get('tagging_verified')} | Magic: {d.get('magic_bytes_valid')} | Staging Clean: {d.get('staging_cleaned')}")
            print()
