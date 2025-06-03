import argparse
import os
import asyncio
from datetime import datetime
import logging

from spotify_sync_lib.config import (
    console, load_app_config, setup_logging, v_print, 
    APP_CONFIG, DEFAULT_SESSION_FILENAME
)
from spotify_sync_lib.session_handler import save_session_data, load_session_data
from services.spotify_api import (
    get_spotify_connection, fetch_spotify_liked_tracks, select_existing_playlist,
    create_new_playlist, 
    add_tracks_to_target_playlist, 
    clean_existing_playlist,
)
from services.local_file_scanner import scan_local_tracks 
from core_logic.track_comparator import compare_tracks, review_uncertain_matches
from core_logic.orphan_processor import process_local_orphans 
from reporting.report_generator import write_results_to_files, display_run_statistics

from rich.panel import Panel
from rich.prompt import Confirm, Prompt
from rich.progress import Progress, SpinnerColumn, TextColumn, BarColumn, TimeElapsedColumn, TimeRemainingColumn

# These will be set after config and args are loaded
SIMILARITY_THRESHOLD = 0 
REVIEW_THRESHOLD = 0

async def run_sync_process(project_root_dir): 
    global SIMILARITY_THRESHOLD, REVIEW_THRESHOLD

    # Load config first, using project_root_dir to find config.json
    load_app_config(project_root_dir) 
    
    # Set up logging after config might have set paths, and after args are parsed for verbosity
    # Argparse happens next, so setup_logging is called after args are parsed.
    
    # Defaults for argparse now come from APP_CONFIG
    parser = argparse.ArgumentParser(description=f"Spotify Library Checker - Modular Edition v2.0")
    parser.add_argument("music_directories", nargs='+', help="Path(s) to your local music director(y/ies).")
    parser.add_argument("--threshold", type=int, default=APP_CONFIG["default_similarity_threshold"], 
                        help=f"Similarity threshold (0-100, config default: {APP_CONFIG['default_similarity_threshold']})")
    parser.add_argument("--review-threshold", type=int, default=APP_CONFIG["default_review_threshold"], 
                        help=f"Review threshold (0-100, config default: {APP_CONFIG['default_review_threshold']})")
    parser.add_argument("-v", "--verbose", action="store_true", help="Enable verbose output")
    parser.add_argument("--session-file", type=str, 
                        default=os.path.join(project_root_dir, DEFAULT_SESSION_FILENAME), 
                        help=f"Filepath for session data (default: {DEFAULT_SESSION_FILENAME} in project dir).")
    parser.add_argument("--force-rescan", action="store_true", help="Force rescan, ignoring session file.")
    parser.add_argument("--no-save-session", action="store_true", help="Disable saving session data.")
    parser.add_argument("--dry-run", action="store_true", help="Perform a dry run; no changes made to Spotify.")
    parser.add_argument("--process-orphans", choices=['display', 'add-to-liked', 'add-to-playlist'], 
                        const='display', nargs='?', 
                        help="Process local tracks not in Spotify Liked Songs. Actions: 'display' (default), 'add-to-liked', 'add-to-playlist'.")
    parser.add_argument("--orphan-playlist-name", type=str, 
                        default="Local Orphans Found (Script)", 
                        help="Default name for a new playlist if 'add-to-playlist' is chosen for orphans and a new playlist is created.")
    args = parser.parse_args()

    # Setup logging now that args.verbose is known
    setup_logging(project_root_dir, args.verbose)

    # Apply CLI args for thresholds, overriding config/script defaults
    SIMILARITY_THRESHOLD = args.threshold 
    REVIEW_THRESHOLD = args.review_threshold
    
    run_stats = {} 

    if args.dry_run:
        console.print(Panel("[bold orange1]DRY RUN MODE ENABLED[/bold orange1]\nNo actual changes will be made to your Spotify account.", expand=False, border_style="orange1"))
        logging.info("Dry run mode activated.")
    
    console.print(Panel(f"Settings: Similarity=[bold]{SIMILARITY_THRESHOLD}%[/bold], Review=[bold]{REVIEW_THRESHOLD}%[/bold]", title="[blue]Current Configuration[/blue]", expand=False))
    logging.info(f"Runtime Settings: Similarity={SIMILARITY_THRESHOLD}%, Review={REVIEW_THRESHOLD}%, Verbose={args.verbose}, DryRun={args.dry_run}, ProcessOrphans={args.process_orphans}")

    local_music_paths = args.music_directories
    local_folder_name_for_playlist = os.path.basename(os.path.normpath(local_music_paths[0])) if len(local_music_paths) == 1 else "Local Collection"
    
    spotify_tracks, local_tracks = [], []
    session_filepath = args.session_file 
    matched_local_filepaths_set = set()

    if not args.force_rescan:
        s_loaded, l_loaded = load_session_data(session_filepath)
        if s_loaded is not None and l_loaded is not None:
            spotify_tracks, local_tracks = s_loaded, l_loaded
        else:
            v_print("Proceeding with full scan (session load failed or file not found).", args.verbose)
    else:
        msg = "Forced rescan. Ignoring any existing session file."
        console.print(f"[yellow]Info: {msg}[/yellow]"); logging.info(msg)

    if not spotify_tracks or not local_tracks: 
        # Connection for reading liked songs
        sp_read = get_spotify_connection(scopes="user-library-read", verbose_flag=args.verbose, project_root_dir=project_root_dir)
        if not sp_read:
            console.print("[red]Exiting due to Spotify connection failure for reading library.[/red]")
            logging.critical("Exiting: Spotify connection failed for reading library.")
            return

        with Progress(SpinnerColumn(), TextColumn("[progress.description]{task.description}"), BarColumn(), TextColumn("{task.completed} of {task.total}"), TimeElapsedColumn(), TimeRemainingColumn(), console=console, transient=False) as progress_manager:
            spotify_fetch_task_id = progress_manager.add_task("Spotify liked init...", total=1, visible=True) 
            local_scan_task_id = progress_manager.add_task("Local scan init...", total=1, visible=True)
            
            v_print("Starting concurrent data fetching...", args.verbose)
            logging.info("Starting concurrent data fetching.")
            
            spotify_tracks_task = asyncio.to_thread(fetch_spotify_liked_tracks, sp_read, progress_manager, spotify_fetch_task_id, args.verbose)
            local_tracks_task = asyncio.to_thread(scan_local_tracks, local_music_paths, progress_manager, local_scan_task_id, args.verbose)
            
            fetched_s_tracks, fetched_l_tracks = await asyncio.gather(spotify_tracks_task, local_tracks_task)
            
            spotify_tracks = fetched_s_tracks if fetched_s_tracks is not None else spotify_tracks
            local_tracks = fetched_l_tracks if fetched_l_tracks is not None else local_tracks
            
        v_print("Finished concurrent data fetching.", args.verbose)
        logging.info("Finished concurrent data fetching.")

        if not args.no_save_session and spotify_tracks and local_tracks : 
             save_session_data(session_filepath, spotify_tracks, local_tracks)
        elif args.no_save_session:
            msg = "Session saving disabled by user."
            console.print(f"[yellow]Info: {msg}[/yellow]"); logging.info(msg)

    run_stats["Spotify Tracks Loaded"] = len(spotify_tracks)
    run_stats["Local Tracks Loaded"] = len(local_tracks)

    if not spotify_tracks:
        console.print("[red]No tracks from Spotify to process. Exiting.[/red]")
        logging.error("No Spotify tracks to process after fetch/load. Exiting.")
        return

    initial_missing_songs, songs_for_review = [], []
    with Progress(TextColumn("[progress.description]{task.description}"), BarColumn(),TextColumn("{task.completed} of {task.total}"), TimeElapsedColumn(), console=console, transient=True) as progress_manager:
        compare_task_id = progress_manager.add_task("Comparison init...", total=1, visible=True)
        initial_missing_songs, songs_for_review = compare_tracks(
            spotify_tracks, local_tracks, progress_manager, compare_task_id, 
            matched_local_filepaths_set, 
            args.verbose, SIMILARITY_THRESHOLD, REVIEW_THRESHOLD
        )
    run_stats["Initial Missing (Spotify not in Local)"] = len(initial_missing_songs)
    run_stats["Tracks for Manual Review"] = len(songs_for_review)
    
    finalized_missing_after_review = []
    if songs_for_review:
        finalized_missing_after_review = review_uncertain_matches(
            songs_for_review, 
            matched_local_filepaths_set, 
            args.verbose
        )
    
    all_missing_songs_final_dict = {s['id']: s for s in initial_missing_songs}
    for s in finalized_missing_after_review: 
        all_missing_songs_final_dict[s['id']] = s
    all_missing_songs_final = list(all_missing_songs_final_dict.values())
    
    run_stats["Final Missing (Spotify not in Local)"] = len(all_missing_songs_final)
    run_stats["Total Matches (Spotify in Local)"] = len(spotify_tracks) - len(all_missing_songs_final)
    
    matches_from_review = 0
    for review_item in songs_for_review: 
        if review_item['spotify_track'].get('review_decision','').startswith('is_match'):
            matches_from_review +=1
    run_stats["Matches Confirmed via Review"] = matches_from_review

    if all_missing_songs_final:
        write_results_to_files(all_missing_songs_final, local_folder_name_for_playlist, is_missing_list=True)
        
        if Confirm.ask("\nProcess playlist for these missing Spotify tracks?", default=False):
            logging.info("User opted to manage playlist for missing Spotify tracks.")
            playlist_scopes = ["user-library-read", "playlist-modify-private", "playlist-read-private", "playlist-modify-public"]
            sp_playlist_mgmt = get_spotify_connection(scopes=playlist_scopes, verbose_flag=args.verbose, project_root_dir=project_root_dir)
            
            if not sp_playlist_mgmt:
                console.print("[red]Could not get necessary permissions for playlist management. Skipping.[/red]")
                logging.error("Playlist management skipped: Spotify connection for playlist scopes failed.")
            else:
                user_id, user_name = "", "User"
                try:
                    from services.spotify_api import spotify_api_call_with_retry # Already imported but to be explicit
                    current_user_info = spotify_api_call_with_retry(lambda: sp_playlist_mgmt.current_user(), args.verbose)
                    if not current_user_info: raise Exception("Failed to retrieve current user for playlist operations.")
                    user_id, user_name = current_user_info['id'], current_user_info.get('display_name', 'User')
                except Exception as e:
                    console.print(f"[red]Error getting user info for playlist management: {e}. Skipping.[/red]")
                    logging.error(f"Playlist management skipped: error getting user info: {e}", exc_info=True)
                
                if user_id: 
                    playlist_id_to_use, playlist_name_to_use = None, APP_CONFIG["default_playlist_name_template"].format("Missing", datetime.now().strftime('%Y-%m-%d'))
                    selected_existing_playlist_obj = None

                    if Confirm.ask("Add to an existing playlist instead of creating a new one?", default=False):
                        selected_existing_playlist_obj = select_existing_playlist(sp_playlist_mgmt, args.verbose)
                        if selected_existing_playlist_obj: 
                            playlist_id_to_use, playlist_name_to_use = selected_existing_playlist_obj['id'], selected_existing_playlist_obj['name']
                            logging.info(f"User selected existing playlist: {playlist_name_to_use}")
                            
                            if Confirm.ask(f"Clean playlist '{playlist_name_to_use}' by removing tracks now found locally?", default=False):
                                logging.info(f"User opted to clean existing playlist '{playlist_name_to_use}'.")
                                with Progress(SpinnerColumn(),TextColumn("[progress.description]{task.description}"), BarColumn(), TextColumn("{task.completed} of {task.total}"), TimeElapsedColumn(), console=console, transient=True) as clean_pm:
                                    clean_task_id = clean_pm.add_task(f"Playlist clean init for '{playlist_name_to_use}'...", total=1)
                                    removed_count = await asyncio.to_thread(
                                        clean_existing_playlist, sp_playlist_mgmt, playlist_id_to_use, playlist_name_to_use,
                                        local_tracks, 
                                        clean_pm, clean_task_id, args.dry_run, args.verbose,
                                        SIMILARITY_THRESHOLD 
                                    )
                                    run_stats[f"Tracks Cleaned from Playlist '{playlist_name_to_use}'"] = removed_count
                            else:
                                logging.info(f"User opted not to clean existing playlist '{playlist_name_to_use}'.")
                        else:
                            console.print("[yellow]No existing playlist selected or cancellation.[/yellow]")
                            logging.info("User cancelled existing playlist selection or none found.")
                    
                    if not playlist_id_to_use: 
                        if Confirm.ask("Create a new playlist for missing tracks?", default=True if not selected_existing_playlist_obj else False):
                            logging.info("User chose to create a new playlist.")
                            default_name = APP_CONFIG["default_playlist_name_template"].format(local_folder_name_for_playlist, datetime.now().strftime('%Y-%m-%d'))
                            p_name = Prompt.ask("New playlist name", default=default_name)
                            p_desc = (f"Tracks for {user_name} missing from '{local_folder_name_for_playlist}'. Checked {datetime.now():%Y-%m-%d %H:%M}." "Playlist generated by Syncify! Compare your local music library here: https://github.com/MadManJohnSmith/Syncify")
                            playlist_id_to_use, _ = create_new_playlist(sp_playlist_mgmt, user_id, p_name, p_desc, args.dry_run, args.verbose)
                            playlist_name_to_use = p_name 
                        else:
                            logging.info("User opted not to create a new playlist.")

                    if playlist_id_to_use: 
                        missing_ids = [s['id'] for s in all_missing_songs_final if s.get('id')]
                        if missing_ids:
                             with Progress(SpinnerColumn(),TextColumn("[progress.description]{task.description}"), BarColumn(), TextColumn("{task.completed} of {task.total}"), TimeElapsedColumn(), console=console, transient=True) as pl_progress:
                                add_id = pl_progress.add_task(f"Playlist add init for '{playlist_name_to_use}'...", total=1)
                                await asyncio.to_thread( 
                                    add_tracks_to_target_playlist, sp_playlist_mgmt, playlist_id_to_use, 
                                    playlist_name_to_use, missing_ids, pl_progress, add_id, 
                                    args.dry_run, args.verbose
                                )
                        else:
                            console.print("  No valid Spotify track IDs for missing songs to add to playlist.")
                            logging.info("No track IDs to add to playlist for missing Spotify tracks.")
        else:
            logging.info("User opted not to create/add to playlist for missing Spotify tracks.")
            
    elif not spotify_tracks: 
        pass 
    else: 
        msg = "No tracks from Spotify appear to be missing from your local library."
        console.print(Panel(f"[bold green]{msg}[/bold green]", expand=False))
        logging.info(msg)

    if args.process_orphans:
        if not local_tracks:
            console.print("[yellow]No local tracks loaded to search for as orphans.[/yellow]")
            logging.info("Process orphans skipped: no local tracks loaded.")
        else:
            local_orphan_tracks = [lt for lt in local_tracks if lt['filepath'] not in matched_local_filepaths_set]
            run_stats["Local Orphan Tracks Identified"] = len(local_orphan_tracks)

            if local_orphan_tracks:
                console.print(Panel(f"[bold blue]Processing {len(local_orphan_tracks)} Local Orphan Tracks on Spotify[/bold blue]", expand=False))
                
                orphan_action_scopes = ['user-library-read'] 
                if args.process_orphans == 'add-to-liked':
                    orphan_action_scopes.append('user-library-modify')
                elif args.process_orphans == 'add-to-playlist':
                    orphan_action_scopes.extend(['playlist-read-private', 'playlist-modify-private', 'playlist-modify-public'])
                
                sp_orphan_processor = get_spotify_connection(scopes=orphan_action_scopes or None, verbose_flag=args.verbose, project_root_dir=project_root_dir)

                if sp_orphan_processor:
                    with Progress(SpinnerColumn(),TextColumn("[progress.description]{task.description}"), BarColumn(), TextColumn("{task.completed} of {task.total}"), TimeElapsedColumn(), console=console, transient=True) as progress_manager:
                        orphan_search_task_id = progress_manager.add_task("Orphan processing init...", total=1) 
                        added_liked, added_pl = await asyncio.to_thread(
                            process_local_orphans, 
                            sp_orphan_processor, local_orphan_tracks, 
                            progress_manager, orphan_search_task_id, 
                            args.dry_run, args.process_orphans, 
                            args.orphan_playlist_name, 
                            args.verbose
                        )
                        run_stats["Orphans Added to Liked Songs"] = added_liked
                        run_stats["Orphans Added to Playlist"] = added_pl
                else:
                    console.print("[red]Could not establish Spotify connection for orphan processing. Skipping.[/red]")
                    logging.error("Orphan processing skipped: Spotify connection failed for required scopes.")
            else:
                console.print("[cyan]No local orphan tracks identified for processing.[/cyan]")
                logging.info("No local orphan tracks identified after main comparison and review.")

    display_run_statistics(run_stats, console)
    console.print(Panel("[bold VIOLET]Script finished.[/bold VIOLET]", expand=False, border_style="violet" ))
    logging.info("Script finished successfully.")