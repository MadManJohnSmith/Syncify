import os
import json
import logging
import re
from rich.console import Console

# --- SCRIPT DIRECTORY & FILE PATHS ---
# PROJECT_ROOT_DIR will be passed from main.py to functions needing it.
# SCRIPT_DIR here refers to the directory of THIS config.py file.
# For locating .env and config.json, we'll use project_root_dir.

DEFAULT_SESSION_FILENAME = ".session_cache.json"
LOG_FILENAME_BASENAME = 'spotify_checker.log'
CONFIG_FILENAME_BASENAME = "config.json"
SPOTIFY_CACHE_BASENAME = ".spotify_user_cache"

# --- RICH CONSOLE ---
console = Console()

# --- APPLICATION CONFIGURATION DEFAULTS ---
# These are script defaults, will be updated by load_app_config from config.json
APP_CONFIG = {
    "normalization_patterns_to_remove_regex": [], # Will be re.compile objects
    "normalization_patterns_to_remove_str": [ # For default in config.json if user doesn't specify
        "\\(official video\\)", "\\[official video\\]", "\\(official lyric video\\)",
        "\\(official audio\\)", "\\[official audio\\]", "\\(lyrics\\)", "\\[lyrics\\]",
        "\\(visualizer\\)", "\\(hd\\)", "\\[hq\\]", "\\(explicit\\)",
        "\\(clean version\\)", "\\(stereo\\)", "\\(mono\\)"
    ],
    "version_keywords": [
        "live", "acoustic", "remix", "remastered", "edit", "version", 
        "deluxe", "extended", "instrumental", "unplugged", "radio edit", 
        "club mix", "anniversary", "mono", "stereo", "original mix", "demo"
    ],
    "supported_formats": ['.mp3', '.flac', '.m4a', '.wav', '.ogg', '.opus', '.aac'],
    "output_links_file": "missing_spotify_links.txt",
    "output_details_file": "missing_spotify_details.txt",
    "default_playlist_name_template": "Missing From {} ({})", # folder_name, date
    "default_similarity_threshold": 85,
    "default_review_threshold": 75,
    "requests_timeout_connect": 10, # Seconds to connect
    "requests_timeout_read": 30,    # Seconds to wait for read
    "api_max_retries": 3,
    "api_initial_retry_delay": 5 # Seconds
}

# --- LOGGING SETUP ---
def setup_logging(project_root_dir, verbose_flag=False):
    level = logging.DEBUG if verbose_flag else logging.INFO
    log_file_path = os.path.join(project_root_dir, LOG_FILENAME_BASENAME)
    
    logging.basicConfig(
        level=level,
        format='%(asctime)s - %(levelname)-8s - [%(module)s.%(funcName)s:%(lineno)d] - %(message)s',
        filename=log_file_path,
        filemode='w' 
    )
    
    # For now, explicit console.print is used, so file logging is the main purpose here.

    if verbose_flag:
        console.print("[yellow]Verbose mode enabled. Detailed logging to file and console.[/yellow]")
        # logging.getLogger().setLevel(logging.DEBUG) # Already set by basicConfig if verbose

    logging.info(f"Logging initialized. Log file: {log_file_path}")


# --- CONFIG LOADING ---
def load_app_config(project_root_dir):
    global APP_CONFIG # Modifying global APP_CONFIG
    
    config_file_path = os.path.join(project_root_dir, CONFIG_FILENAME_BASENAME)
    
    # Start with script defaults (already in APP_CONFIG)
    # Create a deep copy of current APP_CONFIG to use as a base for defaults,
    # ensuring nested lists/dicts are also copied if they were to be modified directly.
    # For this structure, direct assignment of loaded values is fine.
    
    user_config = {}
    try:
        with open(config_file_path, 'r', encoding='utf-8') as f:
            user_config = json.load(f)
        msg = f"Configuration successfully loaded and merged from {config_file_path}"
        logging.info(msg) # Log before console print in case console is not ready (though it is global here)
        console.print(f"[green]{msg}[/green]")
    except FileNotFoundError:
        msg = f"Warning: Configuration file '{config_file_path}' not found. Using internal default configurations."
        console.print(f"[yellow]{msg}[/yellow]"); logging.warning(msg)
    except json.JSONDecodeError:
        msg = f"Error: Could not decode '{config_file_path}'. Check its JSON format. Using internal defaults."
        console.print(f"[red]{msg}[/red]"); logging.error(msg)
    except Exception as e:
        msg = f"Error loading configuration from '{config_file_path}': {e}. Using internal defaults."
        console.print(f"[red]{msg}[/red]"); logging.error(msg, exc_info=True)

    # Merge user_config into APP_CONFIG, user_config takes precedence
    # Special handling for regex patterns
    patterns_str_list = user_config.get("normalization_patterns_to_remove_str", 
                                        APP_CONFIG["normalization_patterns_to_remove_str"])
    APP_CONFIG["normalization_patterns_to_remove_regex"] = [
        re.compile(p, flags=re.IGNORECASE) for p in patterns_str_list
    ]
    APP_CONFIG["normalization_patterns_to_remove_str"] = patterns_str_list # Keep original strings too

    # Update other keys
    for key, default_value in APP_CONFIG.items():
        if key in ["normalization_patterns_to_remove_regex", "normalization_patterns_to_remove_str"]:
            continue 
        if key in user_config:
             APP_CONFIG[key] = user_config[key]
    
    # Ensure numeric values are correctly typed
    for key_numeric in ["default_similarity_threshold", "default_review_threshold", 
                        "requests_timeout_connect", "requests_timeout_read", 
                        "api_max_retries", "api_initial_retry_delay"]:
        if key_numeric in APP_CONFIG:
            try:
                APP_CONFIG[key_numeric] = int(APP_CONFIG[key_numeric])
            except ValueError:
                original_default = { # Re-access original defaults before modification
                    "default_similarity_threshold": 85, "default_review_threshold": 75,
                    "requests_timeout_connect": 10, "requests_timeout_read": 30,    
                    "api_max_retries": 3, "api_initial_retry_delay": 5 
                }
                console.print(f"[red]Warning: Config value for '{key_numeric}' ('{APP_CONFIG[key_numeric]}') is not a valid integer. Using script default: {original_default[key_numeric]}.[/red]")
                logging.warning(f"Config value for '{key_numeric}' ('{APP_CONFIG[key_numeric]}') is not a valid integer. Using script default.")
                APP_CONFIG[key_numeric] = original_default[key_numeric]


def v_print(message, verbose_flag):
    if verbose_flag:
        console.print(f"[dim]VERBOSE:[/dim] {message}")
        logging.debug(f"VERBOSE: {message}") # Log verbose messages as DEBUG