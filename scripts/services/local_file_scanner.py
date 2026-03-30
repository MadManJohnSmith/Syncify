import os
import logging
from tinytag import TinyTag, TinyTagException

from spotify_sync_lib.config import console, APP_CONFIG, v_print
from spotify_sync_lib.text_tools import normalize_text_advanced

def scan_local_tracks(music_dirs, progress, task_id, verbose_flag):
    # music_dirs is now a list of paths
    valid_music_dirs = [d for d in music_dirs if os.path.isdir(d)]
    if not valid_music_dirs:
        msg = f"Error: None of the provided local music directories are valid: {music_dirs}"
        console.print(f"[red]{msg}[/red]"); logging.error(msg)
        progress.update(task_id, description="[red]Local dirs invalid")
        return []
    
    v_print(f"Counting supported files in {len(valid_music_dirs)} director(y/ies)...", verbose_flag)
    logging.info(f"Starting local scan in {valid_music_dirs}. Counting files...")
    
    num_supported_files = 0
    for music_dir in valid_music_dirs:
        for r, d, f_list in os.walk(music_dir):
            for f in f_list:
                if f.lower().endswith(tuple(APP_CONFIG["supported_formats"])): # Use config
                    num_supported_files += 1
    
    msg = f"Found {num_supported_files} potential audio files to scan across specified directories."
    v_print(msg, verbose_flag); logging.info(msg)
    if num_supported_files == 0:
        progress.update(task_id, total=0, completed=0, description="[yellow]No supported local files.")
        return []
    
    progress.update(task_id, total=num_supported_files, description="[blue]Scanning local files...")
    local_tracks_data = []
    tracks_found = 0 # Tracks successfully tagged

    for music_dir in valid_music_dirs: # Iterate through each provided directory
        v_print(f"Scanning directory: {music_dir}", verbose_flag)
        for root, _, files_in_dir in os.walk(music_dir):
            for file_in_root in files_in_dir:
                if file_in_root.lower().endswith(tuple(APP_CONFIG["supported_formats"])): # Use config
                    progress.update(task_id, advance=1)
                    filepath = os.path.join(root, file_in_root)
                    try:
                        tag = TinyTag.get(filepath)
                        if tag and tag.title and tag.artist:
                            local_tracks_data.append({
                                'original_title': tag.title,
                                'original_artist': tag.artist,
                                'album': tag.album or "Unknown Album",
                                'norm_title': normalize_text_advanced(tag.title, is_artist=False),
                                'norm_artist': normalize_text_advanced(tag.artist, is_artist=True),
                                'filepath': filepath
                            })
                            tracks_found += 1
                            if verbose_flag and tracks_found > 0 and tracks_found % 200 == 0:
                                v_print(f"Tagged {tracks_found} local tracks...", verbose_flag)
                    except TinyTagException:
                        v_print(f"TinyTag failed for: {filepath}", verbose_flag)
                        logging.debug(f"TinyTag failed for: {filepath}")
                    except Exception as e:
                        v_print(f"Error processing file {filepath}: {e}", verbose_flag)
                        logging.warning(f"Error processing file {filepath}: {e}", exc_info=True)
    
    msg = f"Finished scanning. Found metadata for {tracks_found} tracks out of {num_supported_files} supported files."
    v_print(msg, verbose_flag); logging.info(msg)
    return local_tracks_data