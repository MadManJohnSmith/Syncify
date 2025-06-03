import logging
import time
from rich.panel import Panel
from rich.table import Table
from rich.prompt import Confirm, Prompt
import rich.box

from spotify_sync_lib.config import console, v_print
from services.spotify_api import (
    spotify_api_call_with_retry, 
    select_existing_playlist, 
    create_new_playlist,
    add_tracks_to_target_playlist,
    get_all_track_ids_in_playlist
)


def process_local_orphans(sp_actions, local_orphan_tracks, progress, task_id, 
                           dry_run_flag, process_orphans_action, 
                           default_orphan_playlist_name, verbose_flag):
    if not local_orphan_tracks:
        console.print("[cyan]No local orphan tracks to search on Spotify.[/cyan]")
        logging.info("No local orphan tracks provided for Spotify search.")
        progress.update(task_id, total=0, completed=0, description="[green]No orphans")
        return 0, 0 

    num_orphans = len(local_orphan_tracks)
    console.print(Panel(f"Processing {num_orphans} local orphan track(s) on Spotify...", title="[blue]Process Orphans[/blue]", expand=False))
    logging.info(f"Starting processing for {num_orphans} local orphan tracks. Action: {process_orphans_action}. Dry run: {dry_run_flag}")
    progress.update(task_id, total=num_orphans, description="[blue]Processing local orphans...")

    added_to_liked_count = 0
    added_to_playlist_count = 0
    
    target_orphan_playlist_id = None
    target_orphan_playlist_name = default_orphan_playlist_name 
    ids_in_target_orphan_playlist_set = set() 

    # Initial playlist setup for "add-to-playlist" action if not dry_run
    # This part is tricky because we only want to ask ONCE for the target playlist.
    # We'll set it up if the action is 'add-to-playlist' and not just 'display'.
    # The actual creation/selection will happen before the first add.
    user_id_for_orphan_playlist = None
    if process_orphans_action == 'add-to-playlist' and not dry_run_flag:
        # Get user_id once if needed for new playlist creation.
        # sp_actions must have user-library-read or similar for current_user()
        try:
            current_user_for_orphan = spotify_api_call_with_retry(lambda: sp_actions.current_user(), verbose_flag)
            if current_user_for_orphan: user_id_for_orphan_playlist = current_user_for_orphan['id']
        except Exception as e:
            logging.warning(f"Could not get current user for orphan playlist creation: {e}. Will prompt if new playlist needed.")


    for i, l_track in enumerate(local_orphan_tracks):
        progress.update(task_id, advance=1)
        v_print(f"Processing orphan: {l_track['original_artist']} - {l_track['original_title']}", verbose_flag)
        query = f"artist:{l_track['norm_artist']} track:{l_track['norm_title']}"
        spotify_hits = []
        try:
            results = spotify_api_call_with_retry(lambda: sp_actions.search(q=query, type="track", limit=5), verbose_flag)
            if results and results['tracks'] and results['tracks']['items']:
                for item in results['tracks']['items']:
                    spotify_hits.append({
                        'title': item['name'], 
                        'artist': ", ".join([a['name'] for a in item['artists']]),
                        'album': item['album']['name'], 
                        'url': item['external_urls']['spotify'], 
                        'id': item['id']
                    })
        except Exception as e:
            msg = f"Error searching Spotify for orphan '{l_track['original_title']}': {e}"
            console.print(f"[red]{msg}[/red]"); logging.error(msg, exc_info=True)
            continue 

        if not spotify_hits:
            v_print(f"No Spotify matches found for local orphan: '{l_track['original_title']}'", verbose_flag)
            logging.info(f"No Spotify matches for orphan: {l_track['original_title']}")
            continue

        console.print(f"\n[bold]Local orphan {i+1}/{num_orphans}:[/bold] [green]{l_track['original_artist']} - {l_track['original_title']}[/green]")
        results_table = Table(title="Potential Spotify Matches", box=rich.box.MINIMAL_HEAVY_HEAD, show_lines=True)
        results_table.add_column("#", width=3); results_table.add_column("Artist"); results_table.add_column("Title"); results_table.add_column("Album"); results_table.add_column("ID", style="dim")
        for idx, hit in enumerate(spotify_hits):
            results_table.add_row(str(idx + 1), hit['artist'], hit['title'], hit['album'], hit['id'])
        console.print(results_table)

        if process_orphans_action != 'display':
            while True: # Loop for user input for this specific orphan
                try:
                    prompt_action_text = process_orphans_action.replace('-', ' ')
                    choice_str = Prompt.ask(f"Select match to '{prompt_action_text}' (1-{len(spotify_hits)}, 0 to skip, 'c' to cancel all orphan processing)", default="0")
                    if choice_str.lower() == 'c':
                        console.print("[yellow]Orphan processing cancelled by user.[/yellow]"); logging.info("Orphan processing cancelled by user.")
                        progress.update(task_id, description="[yellow]Orphan processing cancelled")
                        return added_to_liked_count, added_to_playlist_count 

                    if choice_str == '0': # Skip this orphan
                        logging.info(f"User skipped action for orphan '{l_track['original_title']}'.")
                        break # Process next orphan

                    choice_idx = int(choice_str) - 1
                    if 0 <= choice_idx < len(spotify_hits):
                        selected_spotify_track = spotify_hits[choice_idx]
                        logging.info(f"User selected Spotify track '{selected_spotify_track['title']}' (ID: {selected_spotify_track['id']}) for orphan '{l_track['original_title']}'. Action: {process_orphans_action}")
                        
                        if dry_run_flag:
                            console.print(f"  [DRY RUN] Would perform '{process_orphans_action}' for Spotify track ID {selected_spotify_track['id']}")
                            logging.info(f"[DRY RUN] Action '{process_orphans_action}' for Spotify track ID {selected_spotify_track['id']}")
                            if process_orphans_action == 'add-to-liked': added_to_liked_count +=1
                            elif process_orphans_action == 'add-to-playlist': added_to_playlist_count +=1
                            break # Process next orphan

                        # Perform actual action (if not dry_run)
                        if process_orphans_action == 'add-to-liked':
                            if Confirm.ask(f"Add '{selected_spotify_track['title']}' to Liked Songs?", default=True):
                                try: 
                                    spotify_api_call_with_retry(lambda: sp_actions.current_user_saved_tracks_add(tracks=[selected_spotify_track['id']]), verbose_flag)
                                    console.print(f"  [green]Added '{selected_spotify_track['title']}' to Liked Songs.[/green]"); logging.info(f"Added '{selected_spotify_track['title']}' to Liked Songs."); added_to_liked_count += 1
                                except Exception as e: msg = f"Error adding to Liked Songs: {e}"; console.print(f"  [red]{msg}[/red]"); logging.error(msg, exc_info=True)
                        
                        elif process_orphans_action == 'add-to-playlist':
                            # Determine target playlist if not already set for this session
                            if not target_orphan_playlist_id: 
                                if Confirm.ask(f"Add orphans to a new playlist (default name: '{default_orphan_playlist_name}') or an existing one?", choices=["new", "existing"], default="new") == "existing":
                                    selected_pl_for_orphan = select_existing_playlist(sp_actions, verbose_flag) 
                                    if selected_pl_for_orphan: 
                                        target_orphan_playlist_id = selected_pl_for_orphan['id']
                                        target_orphan_playlist_name = selected_pl_for_orphan['name']
                                        # Fetch existing tracks from this chosen playlist
                                        ids_in_target_orphan_playlist_set = get_all_track_ids_in_playlist(sp_actions, target_orphan_playlist_id, verbose_flag)
                                    else: 
                                        console.print("[yellow]No existing playlist chosen for orphans. Action skipped for this track.[/yellow]"); logging.info("Orphan add-to-playlist skipped: no existing playlist chosen for this session."); break # Break from inner while, go to next orphan
                                else: # Create new
                                    if not user_id_for_orphan_playlist: # Get user_id if not already fetched
                                        current_user_for_orphan = spotify_api_call_with_retry(lambda: sp_actions.current_user(), verbose_flag)
                                        if current_user_for_orphan: user_id_for_orphan_playlist = current_user_for_orphan['id']
                                        else: console.print("[red]Cannot create new playlist: failed to get user info. Action skipped.[/red]"); logging.error("Orphan playlist create failed: no user info for new playlist."); break
                                    
                                    pl_id, _ = create_new_playlist(sp_actions, user_id_for_orphan_playlist, default_orphan_playlist_name, f"Local orphan tracks found on Spotify {datetime.now():%Y-%m-%d}", dry_run_flag, verbose_flag)
                                    if pl_id: 
                                        target_orphan_playlist_id = pl_id
                                        target_orphan_playlist_name = default_orphan_playlist_name # Use the actual name used
                                        ids_in_target_orphan_playlist_set = set() # New playlist is empty
                                    else: 
                                        console.print("[red]Failed to create new playlist for orphans. Action skipped for this track.[/red]"); logging.error("Failed to create new orphan playlist."); break
                            
                            # Now, add to the target_orphan_playlist_id
                            if target_orphan_playlist_id:
                                if selected_spotify_track['id'] in ids_in_target_orphan_playlist_set:
                                    console.print(f"  [yellow]Track '{selected_spotify_track['title']}' is already in playlist '{target_orphan_playlist_name}'. Skipping.[/yellow]")
                                    logging.info(f"Skipped adding duplicate '{selected_spotify_track['title']}' to orphan playlist '{target_orphan_playlist_name}'.")
                                elif Confirm.ask(f"Add '{selected_spotify_track['title']}' to playlist '{target_orphan_playlist_name}'?", default=True):
                                    try: 
                                        # Using playlist_add_items for a single track is fine
                                        spotify_api_call_with_retry(lambda: sp_actions.playlist_add_items(target_orphan_playlist_id, [selected_spotify_track['id']]), verbose_flag)
                                        console.print(f"  [green]Added '{selected_spotify_track['title']}' to playlist '{target_orphan_playlist_name}'.[/green]"); logging.info(f"Added '{selected_spotify_track['title']}' to playlist '{target_orphan_playlist_name}'."); added_to_playlist_count += 1
                                        if not dry_run_flag: ids_in_target_orphan_playlist_set.add(selected_spotify_track['id']) # Update local set
                                    except Exception as e: msg = f"Error adding to playlist '{target_orphan_playlist_name}': {e}"; console.print(f"  [red]{msg}[/red]"); logging.error(msg, exc_info=True)
                        break # Choice made for this orphan, process next orphan
                    else:
                        console.print(f"[yellow]Invalid selection. Please choose a number between 1 and {len(spotify_hits)}, 0 to skip, or 'c' to cancel all.[/yellow]")
                except ValueError:
                    console.print("[yellow]Invalid input. Please enter a number, 0, or 'c'.[/yellow]")
        
        if i < num_orphans - 1 and not dry_run_flag and process_orphans_action != 'display':
            time.sleep(0.5) # Brief pause between processing orphans if actions are taken

    progress.update(task_id, description="[green]Orphan processing complete")
    logging.info(f"Finished processing local orphans. Added to Liked: {added_to_liked_count}, Added to Playlist: {added_to_playlist_count}")
    return added_to_liked_count, added_to_playlist_count