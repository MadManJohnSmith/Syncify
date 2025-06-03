import logging
from collections import defaultdict
from fuzzywuzzy import fuzz
from rich.panel import Panel
from rich.table import Table
from rich.prompt import Prompt
from rich.text import Text
import rich.box

from spotify_sync_lib.config import console, v_print
from spotify_sync_lib.text_tools import extract_version_keywords, generate_block_key

def compare_tracks(spotify_tracks, local_tracks_list, progress, task_id, 
                   matched_local_filepaths_set, verbose_flag, 
                   current_similarity_threshold, current_review_threshold):
    msg = f"Comparing libraries (Similarity: {current_similarity_threshold}%, Review: {current_review_threshold}%)..."
    console.print(f"\n[magenta]{msg}[/magenta]"); logging.info(msg)
    progress.update(task_id, total=len(spotify_tracks), description="[magenta]Comparing libraries (optimized)...")
    
    missing_songs = []
    review_songs_info = []

    local_blocks = defaultdict(list)
    v_print("Pre-processing local tracks into blocks for optimized comparison...", verbose_flag)
    for l_track in local_tracks_list:
        block_key = generate_block_key(l_track['norm_artist'], l_track['norm_title'])
        local_blocks[block_key].append(l_track)
    v_print(f"Created {len(local_blocks)} blocks from {len(local_tracks_list)} local tracks.", verbose_flag)

    for s_track in spotify_tracks: # No tqdm here, progress updated manually
        progress.update(task_id, advance=1)
        best_local_match = None
        highest_score = 0
        s_version_keywords = extract_version_keywords(s_track['original_title'])
        
        s_block_key = generate_block_key(s_track['norm_artist'], s_track['norm_title'])
        candidate_local_tracks = local_blocks.get(s_block_key, [])
        
        if not candidate_local_tracks and verbose_flag:
             v_print(f"Block '{s_block_key}' empty for Spotify track: {s_track['original_artist']} - {s_track['original_title']}", verbose_flag)

        for l_track in candidate_local_tracks: 
            title_similarity = fuzz.ratio(s_track['norm_title'], l_track['norm_title'])
            artist_similarity = fuzz.ratio(s_track['norm_artist'], l_track['norm_artist'])
            match_score = (title_similarity * 0.7) + (artist_similarity * 0.3)
            
            if match_score > highest_score:
                highest_score = match_score
                best_local_match = l_track
        
        match_info = {
            "spotify_track": s_track,
            "best_local_match": best_local_match,
            "score": highest_score,
            "spotify_version_keywords": list(s_version_keywords),
            "local_version_keywords": list(extract_version_keywords(best_local_match['original_title'])) if best_local_match else []
        }
        
        if highest_score >= current_similarity_threshold:
            if best_local_match:
                matched_local_filepaths_set.add(best_local_match['filepath']) 
                s_kws = set(match_info["spotify_version_keywords"])
                l_kws = set(match_info["local_version_keywords"])
                if s_kws != l_kws:
                    version_note = f"Version keywords differ. Spotify: {s_kws or '{none}'}, Local: {l_kws or '{none}'}"
                    s_track['version_note'] = version_note 
                    v_print(f"Match (version note): '{s_track['original_title']}' with '{best_local_match['original_title']}'. {version_note}", verbose_flag)
                else:
                    v_print(f"Match: '{s_track['original_title']}' with '{best_local_match['original_title']}'. Score: {highest_score:.2f}", verbose_flag)
        elif highest_score >= current_review_threshold:
            review_songs_info.append(match_info)
        else:
            missing_songs.append(s_track)
            
    msg = f"Initial comparison: {len(missing_songs)} missing, {len(review_songs_info)} for review."
    v_print(msg, verbose_flag); logging.info(msg)
    progress.update(task_id, description="[green]Comparison complete") 
    return missing_songs, review_songs_info

def review_uncertain_matches(review_songs_info, matched_local_filepaths_set, verbose_flag):
    console.print(Panel("[bold cyan]Manual Review Required[/bold cyan]", expand=False, border_style="cyan"))
    logging.info(f"Starting manual review for {len(review_songs_info)} songs.")
    final_missing_songs = []
    bulk_action = None 
    
    processed_count = 0
    for i, review_info in enumerate(review_songs_info):
        s_track = review_info['spotify_track']
        l_match = review_info['best_local_match']
        score = review_info['score']
        
        current_choice = None

        if bulk_action:
            current_choice = bulk_action[0] 
        else:
            table = Table(title=f"Review Item {i+1}/{len(review_songs_info)}", 
                          show_header=False, box=rich.box.MINIMAL_HEAVY_HEAD)
            table.add_row("[bold magenta]Spotify[/]:", f"{s_track['original_artist']} - {s_track['original_title']} (Album: {s_track['album']})")
            if l_match:
                table.add_row("[bold blue]Local[/]:", f"{l_match['original_artist']} - {l_match['original_title']} (Album: {l_match['album']})")
                table.add_row("[bold]Score[/]:", f"{score:.2f}%")
                s_kws = set(review_info["spotify_version_keywords"])
                l_kws = set(review_info["local_version_keywords"])
                if s_kws != l_kws:
                    table.add_row("[bold yellow]Version Info[/]:", f"Spotify keywords: {s_kws or '{none}'}, Local keywords: {l_kws or '{none}'}")
            else:
                table.add_row("[bold blue]Local[/]:", "No potential match candidate was found with significant score.")
            console.print(table)
            
            prompt_text = Text("  Is this a match? (")
            prompt_text.append("y", style="bold green"); prompt_text.append("es / ")
            prompt_text.append("n", style="bold red"); prompt_text.append("o / ")
            prompt_text.append("s", style="bold yellow"); prompt_text.append("kip / ")
            prompt_text.append("ya", style="bold green"); prompt_text.append("-all / ")
            prompt_text.append("na", style="bold red"); prompt_text.append("-all / ")
            prompt_text.append("sa", style="bold yellow"); prompt_text.append("-all)")
            
            current_choice = Prompt.ask(prompt_text, choices=["y", "n", "s", "ya", "na", "sa"], default="s").lower()

        if current_choice in ["ya", "na", "sa"]:
            bulk_action = current_choice 
            logging.info(f"Bulk action chosen: {bulk_action}")
            current_choice = bulk_action[0] 

        if current_choice == 'n':
            final_missing_songs.append(s_track)
            s_track['review_decision'] = 'not_a_match (bulk)' if bulk_action == 'na' else 'not_a_match'
        elif current_choice == 'y':
            s_track['review_decision'] = 'is_match (bulk)' if bulk_action == 'ya' else 'is_match'
            if l_match:
                matched_local_filepaths_set.add(l_match['filepath']) 
            if l_match and 'version_note' not in s_track and \
               set(review_info["spotify_version_keywords"]) != set(review_info["local_version_keywords"]):
                 s_track['version_note'] = f"Version keyword mismatch (confirmed match). Spotify: {review_info['spotify_version_keywords']}, Local: {review_info['local_version_keywords']}"
        elif current_choice == 's': 
            s_track['review_decision'] = 'skipped (bulk)' if bulk_action == 'sa' else 'skipped'
        
        logging.info(f"Manual review for '{s_track['original_title']}': User chose '{s_track['review_decision']}'.")
        processed_count +=1

    return final_missing_songs