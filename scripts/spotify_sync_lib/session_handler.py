import json
import logging
from datetime import datetime
from .config import console # Use shared console from config module

def save_session_data(filepath, spotify_tracks, local_tracks):
    data_to_save = {
        "spotify_tracks": spotify_tracks,
        "local_tracks": local_tracks,
        "saved_at": datetime.now().isoformat(),
        "version": "1.0" # Second version. First version with modular structure
    }
    try:
        with open(filepath, 'w', encoding='utf-8') as f:
            json.dump(data_to_save, f, indent=2)
        msg = f"Session data saved to {filepath}"
        console.print(f"[green]{msg}[/green]"); logging.info(msg)
    except Exception as e:
        msg = f"Error saving session data to {filepath}: {e}"
        console.print(f"[red]{msg}[/red]"); logging.error(msg, exc_info=True)

def load_session_data(filepath):
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            data = json.load(f)
        # Basic version check example (if ever changed) No surprise if this is not used, but can be useful for future changes
        # file_version = data.get("version")
        # if file_version != "1.0":
        #     console.print(f"[yellow]Warning: Session file version ('{file_version}') differs from script's expected version ('1.0'). Attempting to load anyway.[/yellow]")
        #     logging.warning(f"Session file version mismatch. Expected 1.0, got {file_version}")

        msg = f"Session data loaded from {filepath} (saved at {data.get('saved_at', 'N/A')})"
        console.print(f"[green]{msg}[/green]"); logging.info(msg)
        return data.get("spotify_tracks"), data.get("local_tracks")
    except FileNotFoundError:
        msg = f"Info: Session file {filepath} not found."
        # This is not an error if it's the first run, so use info level.
        console.print(f"[yellow]{msg}[/yellow]"); logging.info(msg)
    except json.JSONDecodeError:
        msg = f"Error: Could not decode session file {filepath}. It might be corrupted."
        console.print(f"[red]{msg}[/red]"); logging.error(msg)
    except Exception as e:
        msg = f"Error loading session data from {filepath}: {e}"
        console.print(f"[red]{msg}[/red]"); logging.error(msg, exc_info=True)
    return None, None