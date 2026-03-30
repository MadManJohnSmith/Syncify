import subprocess
import os
import sys

os.chdir(r'C:\Users\tardis\Documents\Syncify\src-tauri')

print("Running: cargo test --lib -q\n")
result = subprocess.run(
    ['cargo', 'test', '--lib', '-q'],
    capture_output=True,
    text=True,
    encoding='utf-8',
    timeout=300
)

output_lines = result.stdout.split('\n') if result.stdout else []
error_lines = result.stderr.split('\n') if result.stderr else []

print("========== LAST 10 LINES (stdout) ==========")
for line in output_lines[-10:]:
    print(line)

if result.stderr:
    print("\n========== LAST 5 LINES (stderr) ==========")
    for line in error_lines[-5:]:
        print(line)

print(f"\nExit code: {result.returncode}")
sys.exit(result.returncode)
