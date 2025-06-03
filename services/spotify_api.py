import time
import logging
import requests # For specific exceptions
import spotipy
from spotipy.oauth2 import SpotifyOAuth
from rich.panel import Panel
from rich.text import Text
from rich.table import Table
from rich.prompt import Prompt
import rich.box
import os
from collections import defaultdict
from fuzzywuzzy import fuzz

from spotify_sync_lib.config import console, APP_CONFIG, v_print
from spotify_sync_lib.text_tools import normalize_text_advanced, generate_block_key # For processing tracks if needed within this module

# Replace all SPOTIFY_CACHE_PATH with a correct definition
SPOTIFY_CACHE_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '.spotify_user_cache')

# --- API CALL HELPER ---
def spotify_api_call_with_retry(api_call_lambda, verbose_flag):
    max_retries = APP_CONFIG.get("api_max_retries", 3)
    initial_delay = APP_CONFIG.get("api_initial_retry_delay", 5)
    
    for attempt in range(max_retries):
        try:
            return api_call_lambda()
        except (requests.exceptions.ReadTimeout, requests.exceptions.ConnectionError, requests.exceptions.ChunkedEncodingError) as e:
            delay = initial_delay * (2 ** attempt)
            msg = f"Spotify API request failed (attempt {attempt + 1}/{max_retries}): {type(e).__name__} - {str(e)[:100]}. Retrying in {delay}s..."
            logging.warning(msg); console.print(f"[yellow]{msg}[/yellow]")
            if attempt < max_retries - 1: time.sleep(delay)
            else: logging.error(f"Max retries reached for API request after {type(e).__name__}."); raise
        except spotipy.SpotifyException as se:
            delay = initial_delay * (2 ** attempt)
            retry_after_header = se.headers.get('Retry-After') if hasattr(se, 'headers') and se.headers else None

            if se.http_status == 429: # Rate limit
                try: specific_delay = int(retry_after_header); delay = max(specific_delay, delay) 
                except (ValueError, TypeError): pass 
                msg = f"Rate limited by Spotify (HTTP 429). Attempt {attempt + 1}/{max_retries}. Retrying in {delay}s..."
            elif se.http_status >= 500: # Server error
                msg = f"Spotify server error (HTTP {se.http_status}). Attempt {attempt + 1}/{max_retries}. Retrying in {delay}s..."
            else: 
                logging.error(f"Spotify API Error (HTTP {se.http_status}): {se.msg} - URL: {se.url if hasattr(se, 'url') else 'N/A'}")
                raise 
            
            logging.warning(msg); console.print(f"[yellow]{msg}[/yellow]")
            if attempt < max_retries - 1: time.sleep(delay)
            else: logging.error(f"Max retries reached for API request after HTTP {se.http_status}."); raise
    return None 

# --- CONNECTION ---
def get_spotify_connection(scopes="user-library-read", verbose_flag=False, project_root_dir=None):
    # SPOTIFY_CACHE_PATH is already an absolute path based on config_manager.SCRIPT_DIR

    if isinstance(scopes, list): scopes_str = " ".join(sorted(list(set(scopes))))
    elif scopes is None: scopes_str = None
    else: scopes_str = scopes
    
    try:
        # Explicitly get credentials from environment (loaded by dotenv in config.py)
        client_id = os.getenv("SPOTIPY_CLIENT_ID")
        client_secret = os.getenv("SPOTIPY_CLIENT_SECRET")
        redirect_uri = os.getenv("SPOTIPY_REDIRECT_URI")

        logging.debug(f"get_spotify_connection - Scopes: '{scopes_str}'")
        logging.debug(f"  CLIENT_ID from env: {'SET' if client_id else 'NOT SET'}")
        # Not logging secret
        logging.debug(f"  REDIRECT_URI from env: {'SET' if redirect_uri else 'NOT SET'}")

        auth_manager = SpotifyOAuth(
            client_id=client_id,
            client_secret=client_secret,
            redirect_uri=redirect_uri,
            scope=scopes_str, 
            cache_path=SPOTIFY_CACHE_PATH # SPOTIFY_CACHE_PATH from config_manager
        )
        
        sp = spotipy.Spotify(
            auth_manager=auth_manager,
            requests_timeout=(APP_CONFIG["requests_timeout_connect"], APP_CONFIG["requests_timeout_read"])
        )
        
        display_name_for_log = "user"
        if scopes_str: 
            user_info = spotify_api_call_with_retry(lambda: sp.current_user(), verbose_flag=verbose_flag)
            if not user_info: 
                raise spotipy.SpotifyException(401, -1, "Failed to get current user with provided token/scope.")
            display_name_for_log = user_info.get('display_name', 'user') if user_info else 'user'
            msg = f"Successfully authenticated with Spotify as {display_name_for_log} for scope(s): {scopes_str}."
        else:
            # Test with a simple public call if no scopes (Client Credentials)
            spotify_api_call_with_retry(lambda: sp.search(q="test", type="track", limit=1), verbose_flag=verbose_flag) # Test call
            msg = "Spotify connection established (Client Credentials Flow likely)."
            
        console.print(Text(msg, style="cyan")); logging.info(msg)
        return sp
        
    except spotipy.SpotifyOauthError as soe:
        msg = f"Spotify Authentication Error (SpotifyOauthError) for scope(s) '{scopes_str}': {soe}\nEnsure .env has SPOTIPY_CLIENT_ID, SPOTIPY_CLIENT_SECRET, SPOTIPY_REDIRECT_URI and Spotify App redirect URI settings are correct."
        console.print(Panel(Text(msg, style="bold red"), border_style="red")); logging.critical(msg, exc_info=False)
        return None
    except Exception as e:
        msg = f"Spotify Authentication/Connection Error for scope(s) '{scopes_str}': {e}"
        console.print(Panel(Text(msg, style="bold red"), border_style="red")); logging.critical(msg, exc_info=True)
        return None

# --- TRACK FETCHING ---
def fetch_spotify_liked_tracks(sp, progress, task_id, verbose_flag):
    if not sp: return []
    v_print("Starting Spotify library fetch...", verbose_flag); logging.info("Starting Spotify library fetch...")
    spotify_tracks_data = []
    offset, limit, total_tracks_expected = 0, 50, 0
    try:
        results = spotify_api_call_with_retry(lambda: sp.current_user_saved_tracks(limit=1, offset=0), verbose_flag=verbose_flag)
        if results: total_tracks_expected = results.get('total', 0)
        msg = f"Found {total_tracks_expected} tracks in your Spotify library."
        console.print(Text(msg, style="deep_sky_blue1" if console.color_system else "default")); logging.info(msg) 
        if total_tracks_expected == 0:
            progress.update(task_id, total=0, completed=0, description="[green]No Spotify tracks found.")
            return []
        progress.update(task_id, total=total_tracks_expected, description="[green]Fetching Spotify tracks...")
    except Exception as e:
        msg = f"Error fetching initial track count from Spotify: {e}"
        console.print(f"[red]{msg}[/red]"); logging.error(msg, exc_info=True)
        progress.update(task_id, description="[red]Error fetching Spotify tracks")
        return []

    while offset < total_tracks_expected:
        try: 
            results = spotify_api_call_with_retry(lambda: sp.current_user_saved_tracks(limit=limit, offset=offset), verbose_flag=verbose_flag)
        except Exception as e:
            msg = f"Failed to fetch Spotify batch after retries: {e}"
            console.print(f"[red]{msg}[/red]"); logging.error(msg, exc_info=True)
            break 
        
        if not results or not results['items']:
            v_print(f"No more items returned from Spotify at offset {offset}. Expected {total_tracks_expected}.", verbose_flag)
            logging.info(f"No more items from Spotify at offset {offset}. Expected {total_tracks_expected}.")
            break 
        
        page_items = results['items']
        for item in page_items:
            track = item['track']
            if track and track.get('name') and track.get('artists') and track.get('id') and track.get('album'):
                spotify_tracks_data.append({
                    'original_title': track['name'],
                    'original_artist': track['artists'][0]['name'] if track['artists'] else "Unknown",
                    'all_artists_str': ", ".join([a['name'] for a in track['artists']]),
                    'album': track['album']['name'],
                    'norm_title': normalize_text_advanced(track['name'], is_artist=False), 
                    'norm_artist': normalize_text_advanced(track['artists'][0]['name'] if track['artists'] else "Unknown", is_artist=True),
                    'id': track['id'],
                    'url': track['external_urls'].get('spotify', '')
                })
        progress.update(task_id, advance=len(page_items))
        offset += len(page_items)
        v_print(f"Fetched page. Total: {len(spotify_tracks_data)}/{total_tracks_expected}", verbose_flag)
        if not results['next']:
            v_print("Spotify API indicates no next page at current offset.", verbose_flag)
            logging.info("Spotify API indicates no next page at current offset.")
            break
    
    # Check progress.tasks list if task_id is known to be there.
    current_task = next((t for t in progress.tasks if t.id == task_id), None)
    if current_task and len(spotify_tracks_data) >= total_tracks_expected and current_task.completed < total_tracks_expected:
        progress.update(task_id, completed=total_tracks_expected) # Ensure bar finishes

    msg = f"Finished fetching. Loaded {len(spotify_tracks_data)} tracks from Spotify."
    v_print(msg, verbose_flag); logging.info(msg)
    return spotify_tracks_data

def get_all_track_ids_in_playlist(sp, playlist_id, verbose_flag):
    track_ids = set()
    if not playlist_id: return track_ids
    v_print(f"Fetching all track IDs from playlist ID: {playlist_id}", verbose_flag)
    offset, limit = 0, 100 # Max limit for playlist_items is 100
    try:
        while True:
            results = spotify_api_call_with_retry(lambda: sp.playlist_items(playlist_id, fields='items(track(id)),next', limit=limit, offset=offset), verbose_flag)
            if not results or not results['items']: break
            for item in results['items']:
                if item['track'] and item['track']['id']: # Track can be None for local files in playlist not synced
                    track_ids.add(item['track']['id'])
            offset += len(results['items'])
            if not results['next']: break
        v_print(f"Found {len(track_ids)} unique track IDs in playlist {playlist_id}", verbose_flag)
    except Exception as e:
        msg = f"Error fetching all items from playlist {playlist_id}: {e}"
        console.print(f"[red]{msg}[/red]"); logging.error(msg, exc_info=True)
    return track_ids

# --- PLAYLIST MANAGEMENT ---
def select_existing_playlist(sp, verbose_flag):
    v_print("Fetching user playlists...", verbose_flag); logging.info("Fetching user playlists to select from.")
    playlists = []
    try:
        results = spotify_api_call_with_retry(lambda: sp.current_user_playlists(limit=50), verbose_flag)
        if results: playlists.extend(results['items'])
        while results and results['next']: 
            results = spotify_api_call_with_retry(lambda: sp.next(results), verbose_flag) # sp.next handles fetching full URL
            if results: playlists.extend(results['items'])
            else: break # Break if sp.next fails after retries
    except Exception as e:
        msg = f"Error fetching user playlists: {e}"
        console.print(f"[red]{msg}[/red]"); logging.error(msg, exc_info=True)
        return None

    if not playlists:
        console.print("[yellow]No playlists found in your Spotify account.[/yellow]")
        logging.info("No user playlists found.")
        return None

    table = Table(title="Your Spotify Playlists", box=rich.box.MINIMAL_HEAVY_HEAD, show_lines=True)
    table.add_column("#", style="dim", width=4)
    table.add_column("Name", style="bold cyan", overflow="fold")
    table.add_column("Tracks", style="magenta", justify="right")
    table.add_column("ID", style="dim", overflow="ellipsis")

    for i, pl in enumerate(playlists):
        table.add_row(str(i + 1), pl['name'], str(pl['tracks']['total']), pl['id'])
    console.print(table)

    while True:
        try:
            choice_str = Prompt.ask("Enter the number of the playlist to use (or 0 to cancel)")
            choice = int(choice_str)
            if 0 <= choice <= len(playlists):
                if choice == 0: 
                    logging.info("User cancelled existing playlist selection.")
                    return None # Cancel
                selected = playlists[choice - 1]
                logging.info(f"User selected existing playlist: {selected['name']} (ID: {selected['id']})")
                return selected # Return selected playlist object
            console.print(f"[yellow]Invalid selection. Please enter a number between 0 and {len(playlists)}.[/yellow]")
        except ValueError:
            console.print("[yellow]Invalid input. Please enter a number.[/yellow]")


def create_new_playlist(sp, user_id, playlist_name, playlist_description, dry_run_flag, verbose_flag):
    action_title = "[DRY RUN] Playlist Creation" if dry_run_flag else "[cyan]Spotify Playlist Creation[/cyan]"
    console.print(Panel(f"Target Playlist: '{playlist_name}'", title=action_title, expand=False))
    logging.info(f"Attempting to create playlist '{playlist_name}'. Dry run: {dry_run_flag}")

    if dry_run_flag:
        msg = f"  [DRY RUN] Would create playlist '{playlist_name}' with description: '{playlist_description}'"
        console.print(f"[yellow]{msg}[/yellow]"); logging.info(msg)
        # Generate a somewhat unique dummy ID for dry run consistency if needed by other dry run steps HACKKK
        dummy_id = f"dryrun_{playlist_name.replace(' ','_')[:15]}_{int(time.time())%1000}"
        return dummy_id, "dryrun_playlist_url"
    try:
        playlist = spotify_api_call_with_retry(lambda: sp.user_playlist_create(user=user_id, name=playlist_name, public=False, collaborative=False, description=playlist_description), verbose_flag)
        if not playlist: raise Exception("API call for playlist creation returned None after retries.") # Should be caught by spotify_api_call_with_retry
        msg = f"Successfully created playlist: '{playlist['name']}'"
        console.print(f"  [green]{msg}[/green]"); logging.info(msg)
        console.print(f"  Playlist URL: [link={playlist['external_urls']['spotify']}]{playlist['external_urls']['spotify']}[/link]")
        return playlist['id'], playlist['external_urls']['spotify']
    except Exception as e:
        msg = f"Error creating playlist '{playlist_name}': {e}"
        console.print(f"  [red]{msg}[/red]"); logging.error(msg, exc_info=True)
        return None, None

def add_tracks_to_target_playlist(sp, playlist_id, playlist_name, track_ids_to_add, progress, task_id, dry_run_flag, verbose_flag):
    if not track_ids_to_add:
        progress.update(task_id, description=f"[green]'{playlist_name}': No new tracks to add.")
        console.print(f"  No new tracks to add to '{playlist_name}'."); logging.info(f"No new tracks to add to '{playlist_name}'.")
        return
    
    actual_ids_to_add = list(track_ids_to_add) 
    if not dry_run_flag:
        v_print(f"Checking for existing tracks in playlist '{playlist_name}' before adding...", verbose_flag)
        existing_track_ids_in_playlist = get_all_track_ids_in_playlist(sp, playlist_id, verbose_flag)
        
        original_count = len(actual_ids_to_add)
        actual_ids_to_add = [tid for tid in actual_ids_to_add if tid not in existing_track_ids_in_playlist]
        num_duplicates_skipped = original_count - len(actual_ids_to_add)

        if num_duplicates_skipped > 0:
            msg = f"  Skipped {num_duplicates_skipped} track(s) already present in '{playlist_name}'."
            console.print(f"[yellow]{msg}[/yellow]"); logging.info(msg)
        if not actual_ids_to_add:
            progress.update(task_id, completed=0, total=0, description=f"[green]'{playlist_name}' up-to-date") # Update progress bar even if no tracks added
            console.print(f"  All {original_count} candidate tracks already in '{playlist_name}'. Nothing new to add."); logging.info(f"All tracks already in '{playlist_name}'.")
            return
            
    num_tracks = len(actual_ids_to_add)
    action_desc = f"[DRY RUN] Add to '{playlist_name}'" if dry_run_flag else f"[yellow]Add to '{playlist_name}'..."
    logging.info(f"Adding {num_tracks} new tracks to playlist '{playlist_name}' (ID: {playlist_id}). Dry run: {dry_run_flag}")
    
    if dry_run_flag:
        msg = f"  [DRY RUN] Would add {num_tracks} new tracks to playlist '{playlist_name}' (ID: '{playlist_id}')"
        console.print(f"[yellow]{msg}[/yellow]"); logging.info(msg)
        progress.update(task_id, total=num_tracks, completed=num_tracks, description="[DRY RUN] Add simulated")
        return

    progress.update(task_id, total=num_tracks, description=action_desc) # Set total for real run
    batch_size, added_count = 100, 0
    for i in range(0, num_tracks, batch_size):
        batch = actual_ids_to_add[i : i + batch_size]
        try:
            spotify_api_call_with_retry(lambda: sp.playlist_add_items(playlist_id, batch), verbose_flag)
            added_count += len(batch); progress.update(task_id, advance=len(batch))
            v_print(f"Added batch. Total added: {added_count}", verbose_flag)
            if num_tracks > batch_size and i + batch_size < num_tracks: time.sleep(1)
        except Exception as e:
            msg = f"Error adding batch to '{playlist_name}': {e}"; console.print(f"  [red]{msg}[/red]"); logging.error(msg, exc_info=True)
            break # Stop if a batch fails
    
    final_desc = f"[green]'{playlist_name}': {added_count}/{num_tracks} added!" if added_count == num_tracks else f"[yellow]'{playlist_name}': {added_count}/{num_tracks} (partial)"
    progress.update(task_id, description=final_desc)
    logging.info(f"Finished adding to '{playlist_name}'. Added {added_count}/{num_tracks} new tracks.")


def clean_existing_playlist(sp, playlist_id, playlist_name, local_tracks_list, progress, task_id, dry_run_flag, verbose_flag, current_similarity_threshold):
    console.print(Panel(f"Cleaning playlist '{playlist_name}' (ID: {playlist_id})", title="[blue]Playlist Cleaning[/blue]", expand=False))
    logging.info(f"Starting cleaning of playlist '{playlist_name}'. Dry run: {dry_run_flag}")

    playlist_spotify_tracks_raw = []
    offset, limit, total_playlist_tracks = 0, 100, 0
    try:
        results = spotify_api_call_with_retry(lambda: sp.playlist_items(playlist_id, limit=limit, offset=offset, fields='items(track(name,artists(name),album(name),id,external_urls)),next,total'), verbose_flag)
        if not results: raise Exception("Failed to fetch initial playlist items.")
        total_playlist_tracks = results.get('total', 0)
        if results['items']: playlist_spotify_tracks_raw.extend(item['track'] for item in results['items'] if item['track'] and item['track'].get('id')) # Ensure track and ID exist
        
        progress.update(task_id, total=total_playlist_tracks, description=f"[blue]Fetching from '{playlist_name}'...")
        progress.update(task_id, advance=len(results['items'] or []))
        offset += len(results['items'] or [])

        while results and results['next'] and offset < total_playlist_tracks:
            results = spotify_api_call_with_retry(lambda: sp.next(results), verbose_flag)
            if not results: break
            if results['items']: playlist_spotify_tracks_raw.extend(item['track'] for item in results['items'] if item['track'] and item['track'].get('id'))
            progress.update(task_id, advance=len(results['items'] or [])); offset += len(results['items'] or [])
        
        current_task_obj = progress._tasks.get(task_id) # Access task via internal _tasks dict
        if current_task_obj and len(playlist_spotify_tracks_raw) >= total_playlist_tracks and current_task_obj.completed < total_playlist_tracks:
             progress.update(task_id, completed=total_playlist_tracks) # Mark as complete if all fetched
    except Exception as e:
        msg = f"Error fetching all tracks from playlist '{playlist_name}': {e}"
        console.print(f"[red]{msg}[/red]"); logging.error(msg, exc_info=True)
        progress.update(task_id, description=f"[red]Error fetching from '{playlist_name}'")
        return 0 

    if not playlist_spotify_tracks_raw:
        console.print(f"[yellow]Playlist '{playlist_name}' is empty or tracks could not be fetched. No cleaning needed.[/yellow]")
        logging.info(f"Playlist '{playlist_name}' empty or unreadable. No cleaning.")
        progress.update(task_id, description=f"[green]'{playlist_name}' empty/no cleaning")
        return 0

    v_print(f"Fetched {len(playlist_spotify_tracks_raw)} tracks from '{playlist_name}'. Now checking against local library...", verbose_flag)
    progress.update(task_id, completed=0, total=len(playlist_spotify_tracks_raw), description=f"[blue]Analyzing '{playlist_name}' tracks...")

    local_blocks = defaultdict(list)
    for l_track in local_tracks_list: local_blocks[generate_block_key(l_track['norm_artist'], l_track['norm_title'])].append(l_track)
    
    track_ids_to_remove = []
    for pl_track in playlist_spotify_tracks_raw:
        progress.update(task_id, advance=1)
        if not pl_track or not pl_track.get('id'): 
            v_print(f"Skipping malformed/local track data from playlist: {pl_track}", verbose_flag); continue # Skip if no ID (e.g. local file in playlist)

        pl_norm_title = normalize_text_advanced(pl_track['name'])
        pl_norm_artist = normalize_text_advanced(pl_track['artists'][0]['name'] if pl_track['artists'] else "Unknown", is_artist=True)
        block_key = generate_block_key(pl_norm_artist, pl_norm_title)
        candidate_locals = local_blocks.get(block_key, [])
        found_locally = False
        for l_track in candidate_locals:
            title_sim, artist_sim = fuzz.ratio(pl_norm_title, l_track['norm_title']), fuzz.ratio(pl_norm_artist, l_track['norm_artist'])
            match_score = (title_sim * 0.7) + (artist_sim * 0.3)
            if match_score >= current_similarity_threshold: 
                found_locally = True; v_print(f"Playlist track '{pl_track['name']}' found locally as '{l_track['original_title']}'. Mark for removal.", verbose_flag); break
        if found_locally: track_ids_to_remove.append(pl_track['id'])

    removed_count = 0
    if track_ids_to_remove:
        logging.info(f"{len(track_ids_to_remove)} tracks in playlist '{playlist_name}' found locally and will be removed. Dry run: {dry_run_flag}")
        if dry_run_flag:
            console.print(f"  [DRY RUN] Would remove {len(track_ids_to_remove)} tracks from playlist '{playlist_name}'.")
            for track_id in track_ids_to_remove: logging.info(f"[DRY RUN] Would remove ID: {track_id}")
            removed_count = len(track_ids_to_remove)
        else:
            for i in range(0, len(track_ids_to_remove), 100):
                batch_ids_uris = [{'uri': f"spotify:track:{tid}"} for tid in track_ids_to_remove[i:i+100]] # Use URI format
                try:
                    spotify_api_call_with_retry(lambda: sp.playlist_remove_specific_occurrences_of_items(playlist_id, batch_ids_uris), verbose_flag)
                    removed_count += len(batch_ids_uris)
                    v_print(f"Removed batch of {len(batch_ids_uris)} from '{playlist_name}'. Total removed: {removed_count}", verbose_flag)
                except Exception as e:
                    msg = f"Error removing tracks batch from playlist '{playlist_name}': {e}"
                    console.print(f"  [red]{msg}[/red]"); logging.error(msg, exc_info=True)
                    break 
            console.print(f"  [green]Removed {removed_count} tracks from playlist '{playlist_name}'.[/green]")
    else:
        console.print(f"  [green]No tracks in playlist '{playlist_name}' needed removal (none found in local library).[/green]")
        logging.info(f"No tracks in playlist '{playlist_name}' found locally during cleaning.")

    progress.update(task_id, description=f"[green]'{playlist_name}' cleaning complete")
    return removed_count