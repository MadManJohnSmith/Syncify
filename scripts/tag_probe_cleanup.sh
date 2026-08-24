#!/usr/bin/env bash
# ==============================================================================
# Syncify — Symfonium Tag Probe Batch Cleanup
# ==============================================================================
# Cleans up temporary synthetic probe tags from Phase 1 and Phase 2 test batches:
# LANGUAGE, STYLE, ALBUMSTYLE, TRACKSTYLE, MOOD, ALBUMMOOD, TRACKMOOD,
# TAGS, ALBUMTAGS, ARTISTTAGS, ARTIST_TAG, ARTISTS_TAGS, TRACKTAGS, TRACK_TAG,
# COMPILATION, GROUPING, OCCASION, MEDIA, MUSICTYPE, RELEASETYPE.
# ==============================================================================
set -euo pipefail

PROBE1="/tmp/tag_audit/probe_files.txt"
PROBE2="/tmp/tag_audit/probe_files_phase2.txt"

ALL_FILES=()

if [ -f "$PROBE1" ]; then
    while IFS= read -r line; do
        [ -n "$line" ] && ALL_FILES+=("$line")
    done < "$PROBE1"
fi

if [ -f "$PROBE2" ]; then
    while IFS= read -r line; do
        [ -n "$line" ] && ALL_FILES+=("$line")
    done < "$PROBE2"
fi

echo "=== S179 Probe Cleanup: Removing synthetic probe tags from ${#ALL_FILES[@]} files ==="

for FILE in "${ALL_FILES[@]}"; do
    if [ ! -f "$FILE" ]; then
        echo "  [SKIP] File not found: $FILE"
        continue
    fi

    echo "Cleaning: $FILE"
    metaflac --remove-tag=TRACKTAGS \
             --remove-tag=ARTIST_TAG \
             --remove-tag=TRACK_TAG \
             --remove-tag=ARTISTS_TAGS \
             --remove-tag=ARTISTTAGS \
             --remove-tag=MEDIA \
             --remove-tag=MUSICTYPE \
             --remove-tag=RELEASETYPE \
             --remove-tag=OCCASION \
             --remove-tag=STYLE \
             --remove-tag=ALBUMSTYLE \
             --remove-tag=TRACKSTYLE \
             --remove-tag=MOOD \
             --remove-tag=ALBUMMOOD \
             --remove-tag=TRACKMOOD \
             --remove-tag=TAGS \
             --remove-tag=ALBUMTAGS \
             --remove-tag=COMPILATION \
             --remove-tag=GROUPING \
             "$FILE"

    # For Max Richter Leftovers tracks, restore ALBUMARTIST=Max Richter
    if [[ "$FILE" == *"The Leftovers"* ]]; then
        metaflac --remove-tag=ALBUMARTIST --set-tag="ALBUMARTIST=Max Richter" "$FILE"
    fi
done

echo ""
echo "=== POST-CLEANUP VERIFICATION ==="
for FILE in "${ALL_FILES[@]}"; do
    if [ -f "$FILE" ]; then
        echo "--- $(basename "$FILE") ---"
        tags=$(metaflac --show-tag=TRACKTAGS \
                        --show-tag=ARTIST_TAG \
                        --show-tag=TRACK_TAG \
                        --show-tag=ARTISTS_TAGS \
                        --show-tag=ARTISTTAGS \
                        --show-tag=OCCASION \
                        "$FILE")
        if [ -z "$tags" ]; then
            echo "  [CLEAN] No synthetic probe tags remaining."
        else
            echo "  [WARN] Remaining tags: $tags"
        fi
    fi
done

echo ""
echo "=== Probe Cleanup Complete ==="
