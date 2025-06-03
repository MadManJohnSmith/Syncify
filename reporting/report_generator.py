import os
import logging
from rich.panel import Panel
from rich.table import Table
import rich.box # For table box styles

from spotify_sync_lib.config import console, APP_CONFIG 

def write_results_to_files(tracks_to_output, local_music_dir_name="your local library", is_missing_list=True):
    count = len(tracks_to_output)
    status_message = "missing tracks" if is_missing_list else "tracks with notes" 
    
    console.print(Panel(f"Outputting {count} {status_message} to files.", title="[green]File Output[/green]", expand=False))
    logging.info(f"Writing {count} {status_message} to output files.")

    # Get filenames from APP_CONFIG (which was loaded from config.json or defaults)
    links_file = APP_CONFIG['output_links_file']
    details_file_template = APP_CONFIG['output_details_file']

    if is_missing_list: 
        with open(links_file, 'w', encoding='utf-8') as f_links:
            for track in tracks_to_output:
                if track.get('url'):
                    f_links.write(track['url'] + '\n')
        console.print(f"  Spotify links of missing tracks written to: [italic link=file://{os.path.abspath(links_file)}]{links_file}[/italic link]")
        logging.info(f"Links-only file written to {links_file}")

    details_filename_actual = details_file_template if is_missing_list else "spotify_tracks_annotated_report.txt"

    with open(details_filename_actual, 'w', encoding='utf-8') as f_details:
        f_details.write("Spotify URL\tTitle\tArtists\tAlbum\tNote\n") 
        for track in tracks_to_output:
            note = track.get('version_note', '')
            review_decision = track.get('review_decision', '')
            if review_decision:
                note += f" (Review: {review_decision})"
            
            if track.get('url'): 
                f_details.write(f"{track['url']}\t{track['original_title']}\t{track['all_artists_str']}\t{track['album']}\t{note.strip()}\n")
    
    console.print(f"  Detailed report written to: [italic link=file://{os.path.abspath(details_filename_actual)}]{details_filename_actual}[/italic link]")
    logging.info(f"Detailed report written to {details_filename_actual}")


def display_run_statistics(stats, console_instance): # console_instance is passed from main
    stats_table = Table(title="📊 Run Statistics", show_header=True, header_style="bold magenta", box=rich.box.ROUNDED, padding=(0,1))
    stats_table.add_column("Metric", style="dim cyan", width=40) 
    stats_table.add_column("Value", style="bold white")
    
    for key, value in stats.items():
        stats_table.add_row(key, str(value))
    
    console_instance.print(stats_table)
    logging.info(f"Run Statistics: {stats}")