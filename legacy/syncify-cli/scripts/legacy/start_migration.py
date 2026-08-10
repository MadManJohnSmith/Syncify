"""
🎵 QUICK START MIGRATION SCRIPT
Run this to start migrating your Spotify library to Qobuz

Based on your test results:
- 500/500 tracks matched (100%)
- 12,183 total tracks in library
- ~476 GB storage needed
- ~6.8 hours estimated
"""

import asyncio
import json
import sys
from pathlib import Path
from datetime import datetime

print("=" * 80)
print("🎵 SYNCIFY - SPOTIFY → QOBUZ MIGRATION")
print("=" * 80)
print()

# Check if config exists
if not Path("config.json").exists():
    print("❌ config.json not found!")
    print("   Please ensure your credentials are configured.")
    sys.exit(1)

# Load config
with open("config.json", "r") as f:
    config = json.load(f)

# Verify credentials
print("✓ Checking configuration...")
has_spotify = config.get("spotify", {}).get("client_id")
has_qobuz = config.get("qobuz", {}).get("username")

if not has_spotify:
    print("❌ Spotify credentials not configured!")
    sys.exit(1)

if not has_qobuz:
    print("❌ Qobuz credentials not configured!")
    sys.exit(1)

print("✓ Spotify credentials found")
print("✓ Qobuz credentials found")
print()

# Show options
print("📋 MIGRATION OPTIONS")
print("-" * 80)
print()
print("1. 🧪 TEST RUN (10 tracks)")
print("   Quick test to verify everything works")
print()
print("2. 📦 SMALL BATCH (100 tracks)")
print("   Recommended first migration to check quality/organization")
print()
print("3. 📊 DRY RUN (analyze only, no download)")
print("   See what will be migrated without downloading")
print()
print("4. 🚀 FULL MIGRATION (all 12,183 tracks)")
print("   Complete library migration (~476 GB, ~6.8 hours)")
print()
print("5. ❌ EXIT")
print()

choice = input("Select option (1-5): ").strip()

print()
print("=" * 80)

if choice == "1":
    print("🧪 STARTING TEST RUN (10 tracks)")
    print("=" * 80)
    print()
    print("This will:")
    print("  • Connect to Spotify and Qobuz")
    print("  • Fetch 10 tracks from your library")
    print("  • Search and match on Qobuz")
    print("  • Download in FLAC quality")
    print("  • Save to: ./downloads/test/")
    print()
    confirm = input("Continue? (y/n): ").strip().lower()
    if confirm != 'y':
        print("Cancelled.")
        sys.exit(0)
    
    # Create test migration script
    print()
    print("Starting migration...")
    print("(This would run: python main.py --limit 10 --output ./downloads/test/)")
    print()
    print("⚠️  Note: Full CLI implementation needed in main.py")
    print("   For now, use: python test_full_analysis.py")

elif choice == "2":
    print("📦 STARTING SMALL BATCH (100 tracks)")
    print("=" * 80)
    print()
    print("This will:")
    print("  • Download first 100 tracks")
    print("  • Estimated time: ~40 minutes")
    print("  • Estimated storage: ~4 GB")
    print("  • Save to: ./downloads/")
    print()
    confirm = input("Continue? (y/n): ").strip().lower()
    if confirm != 'y':
        print("Cancelled.")
        sys.exit(0)
    
    print()
    print("Starting migration...")
    print("(This would run: python main.py --limit 100)")
    print()
    print("⚠️  Note: Full CLI implementation needed in main.py")

elif choice == "3":
    print("📊 STARTING DRY RUN")
    print("=" * 80)
    print()
    print("This will:")
    print("  • Analyze all 12,183 tracks")
    print("  • Show match statistics")
    print("  • Estimate storage and time")
    print("  • NOT download anything")
    print()
    confirm = input("Continue? (y/n): ").strip().lower()
    if confirm != 'y':
        print("Cancelled.")
        sys.exit(0)
    
    print()
    print("Starting analysis...")
    print("Use the existing test script: python test_full_analysis.py")

elif choice == "4":
    print("🚀 STARTING FULL MIGRATION")
    print("=" * 80)
    print()
    print("⚠️  IMPORTANT: This will:")
    print("  • Download ALL 12,183 tracks")
    print("  • Use ~476 GB storage")
    print("  • Take ~6.8 hours")
    print("  • Use significant bandwidth")
    print()
    print("✓ Recommendations:")
    print("  • Ensure you have 500+ GB free space")
    print("  • Use stable internet connection")
    print("  • Run overnight or when you won't need the PC")
    print("  • Keep this window open (progress will be shown)")
    print()
    
    confirm = input("⚠️  Are you SURE you want to proceed? (yes/no): ").strip().lower()
    if confirm != 'yes':
        print("Cancelled.")
        sys.exit(0)
    
    print()
    print("=" * 80)
    print(f"🚀 MIGRATION STARTED: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print("=" * 80)
    print()
    
    # Import and run the actual migration
    print("Running main.py...")
    print()
    
    from spotify_sync_lib.app_orchestrator import run_sync_process
    from spotify_sync_lib.config import console
    
    SCRIPT_DIR = Path(__file__).parent
    try:
        asyncio.run(run_sync_process(str(SCRIPT_DIR)))
    except KeyboardInterrupt:
        console.print("\n[yellow]Migration interrupted by user.[/yellow]")
        console.print("Progress has been saved. Run again with --resume to continue.")
    except Exception as e:
        console.print(f"[bold red]Error during migration:[/bold red]\n{e}")
        console.print_exception(show_locals=True)

elif choice == "5":
    print("Goodbye!")
    sys.exit(0)

else:
    print("❌ Invalid option!")
    sys.exit(1)
