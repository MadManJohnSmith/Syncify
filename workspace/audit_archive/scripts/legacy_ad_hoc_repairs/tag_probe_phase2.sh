#!/usr/bin/env bash
# ==============================================================================
# Syncify — Symfonium Tag Probe Batch (Phase 2)
# ==============================================================================
# Tests unresolved Symfonium metadata facets:
# 1. Track tags & Artist tags candidate Vorbis tags (TRACKTAGS, ARTIST_TAG, TRACK_TAG, ARTISTS_TAGS, SingleValue TAGS).
# 2. Real Compilation handling (COMPILATION=1, ALBUMARTIST=Various Artists across 3 tracks).
# 3. Media type & Release type candidate Vorbis tags (MEDIA, MUSICTYPE, RELEASETYPE = Soundtrack).
# ==============================================================================
set -euo pipefail

PROBE_LIST="/tmp/tag_audit/probe_files_phase2.txt"

if [ ! -f "$PROBE_LIST" ]; then
    echo "ERROR: Probe file list $PROBE_LIST not found!"
    exit 1
fi

mapfile -t FILES < "$PROBE_LIST"
if [ "${#FILES[@]}" -ne 6 ]; then
    echo "ERROR: Expected exactly 6 files in $PROBE_LIST, found ${#FILES[@]}"
    exit 1
fi

echo "=== S178 Phase 2 Probe: Tagging 6 Sample FLACs for Symfonium Validation ==="

for idx in "${!FILES[@]}"; do
    FILE="${FILES[$idx]}"
    TRACK_NUM=$((idx + 1))
    echo ""
    echo "[$TRACK_NUM/6] Processing: $FILE"
    
    if [ ! -f "$FILE" ]; then
        echo "  [ERROR] File does not exist: $FILE"
        exit 1
    fi

    # 1. Remove previous probe tags if any to ensure clean test
    metaflac --remove-tag=TRACKTAGS \
             --remove-tag=ARTIST_TAG \
             --remove-tag=TRACK_TAG \
             --remove-tag=ARTISTS_TAGS \
             --remove-tag=MEDIA \
             --remove-tag=MUSICTYPE \
             --remove-tag=RELEASETYPE \
             --remove-tag=COMPILATION \
             "$FILE"

    # 2. Universal Phase 2 Candidate Tags: Track & Artist tags variants
    metaflac --set-tag="TRACKTAGS=ProbeTrackTag" \
             --set-tag="ARTIST_TAG=ProbeArtistTag" \
             --set-tag="TRACK_TAG=ProbeTrackTag2" \
             --set-tag="ARTISTS_TAGS=ProbeArtistTag2" \
             "$FILE"

    # 3. Specific test variations:
    case "$TRACK_NUM" in
        1)
            # Track 1 of Compilation + Media Type Variant A
            echo "  Applying: COMPILATION=1, ALBUMARTIST=Various Artists, MEDIA=Soundtrack, MUSICTYPE=Album, RELEASETYPE=Soundtrack"
            metaflac --set-tag="COMPILATION=1" \
                     --remove-tag=ALBUMARTIST --set-tag="ALBUMARTIST=Various Artists" \
                     --set-tag="MEDIA=Soundtrack" \
                     --set-tag="MUSICTYPE=Album" \
                     --set-tag="RELEASETYPE=Soundtrack" \
                     "$FILE"
            ;;
        2)
            # Track 2 of Compilation + Media Type Variant B
            echo "  Applying: COMPILATION=1, ALBUMARTIST=Various Artists, MEDIA=Soundtrack, MUSICTYPE=Soundtrack, RELEASETYPE=Soundtrack"
            metaflac --set-tag="COMPILATION=1" \
                     --remove-tag=ALBUMARTIST --set-tag="ALBUMARTIST=Various Artists" \
                     --set-tag="MEDIA=Soundtrack" \
                     --set-tag="MUSICTYPE=Soundtrack" \
                     --set-tag="RELEASETYPE=Soundtrack" \
                     "$FILE"
            ;;
        3)
            # Track 3 of Compilation
            echo "  Applying: COMPILATION=1, ALBUMARTIST=Various Artists"
            metaflac --set-tag="COMPILATION=1" \
                     --remove-tag=ALBUMARTIST --set-tag="ALBUMARTIST=Various Artists" \
                     "$FILE"
            ;;
        4)
            # Single-value TAGS test: Does a single tag descend to Track Tags?
            echo "  Applying: TAGS=SingleValue (isolated single value)"
            metaflac --remove-tag=TAGS \
                     --remove-tag=ALBUMTAGS \
                     --set-tag="TAGS=SingleValue" \
                     "$FILE"
            ;;
        5|6)
            echo "  Applying: Standard Phase 2 Track/Artist candidate tags"
            ;;
    esac

    echo "  [OK] Tags written successfully."
done

echo ""
echo "=== POST-WRITE VERIFICATION ==="
for idx in "${!FILES[@]}"; do
    FILE="${FILES[$idx]}"
    TRACK_NUM=$((idx + 1))
    echo ""
    echo "--- File $TRACK_NUM: $(basename "$FILE") ---"
    metaflac --show-tag=COMPILATION \
             --show-tag=ALBUMARTIST \
             --show-tag=TRACKTAGS \
             --show-tag=ARTIST_TAG \
             --show-tag=TRACK_TAG \
             --show-tag=ARTISTS_TAGS \
             --show-tag=TAGS \
             --show-tag=MEDIA \
             --show-tag=MUSICTYPE \
             --show-tag=RELEASETYPE \
             "$FILE"
done

echo ""
echo "=== Phase 2 Probe Complete ==="
