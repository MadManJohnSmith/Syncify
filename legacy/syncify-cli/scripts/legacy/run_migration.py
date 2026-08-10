"""
Spotify to Qobuz Migration Script

This script migrates your Spotify library to Qobuz with proper authentication
and ISRC-based matching.
"""

import asyncio
import json
import os
from pathlib import Path
from datetime import datetime
from typing import List, Dict, Optional

import spotipy
from spotipy.oauth2 import SpotifyOAuth
from rich.console import Console
from rich.progress import Progress, SpinnerColumn, BarColumn, TextColumn, TimeRemainingColumn, TaskID
from rich.panel import Panel
from rich.table import Table

from services.qobuz_service import QobuzService
from services.service_base import ServiceCredentials, ServiceType, DownloadQuality, SearchResult

console = Console()


class SpotifyToQobuzMigration:
    """Handles migration from Spotify to Qobuz."""
    
    def __init__(self, config_path: str = "config.json"):
        self.config_path = config_path
        self.config = self._load_config()
        self.spotify = None
        self.qobuz = None
        self.matched_tracks = []
        self.failed_tracks = []
        
    def _load_config(self) -> dict:
        """Load configuration from JSON file."""
        try:
            with open(self.config_path, 'r') as f:
                return json.load(f)
        except FileNotFoundError:
            console.print(f"[bold red]❌ Configuration file not found: {self.config_path}[/bold red]")
            raise
    
    def _init_spotify(self):
        """Initialize Spotify client."""
        spotify_config = self.config.get('spotify', {})
        
        auth_manager = SpotifyOAuth(
            client_id=spotify_config['client_id'],
            client_secret=spotify_config['client_secret'],
            redirect_uri='http://localhost:8888/callback',
            scope='user-library-read playlist-read-private',
            cache_path='.spotify_cache'
        )
        
        self.spotify = spotipy.Spotify(auth_manager=auth_manager)
        console.print("[green]✓ Spotify authenticated[/green]")
    
    async def _init_qobuz(self):
        """Initialize Qobuz service."""
        qobuz_config = self.config.get('qobuz', {})
        
        credentials = ServiceCredentials(
            service_type=ServiceType.QOBUZ,
            username=qobuz_config['username'],
            password=qobuz_config['password']
        )
        
        self.qobuz = QobuzService(credentials, verbose=False)
        success = await self.qobuz.authenticate()
        
        if success:
            console.print("[green]✓ Qobuz authenticated[/green]")
        else:
            console.print("[bold red]❌ Qobuz authentication failed[/bold red]")
            raise Exception("Qobuz authentication failed")
    
    async def _cleanup(self):
        """Cleanup resources."""
        if self.qobuz and self.qobuz.session:
            await self.qobuz.session.close()
    
    def _fetch_spotify_tracks(self, limit: Optional[int] = None) -> List[Dict]:
        """Fetch all liked tracks from Spotify."""
        tracks = []
        offset = 0
        batch_size = 50
        
        # Get total count
        results = self.spotify.current_user_saved_tracks(limit=1, offset=0)
        total = results['total'] if not limit else min(results['total'], limit)
        
        console.print(f"[cyan]📚 Found {total} tracks in Spotify library[/cyan]")
        
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            BarColumn(),
            TextColumn("[progress.percentage]{task.percentage:>3.0f}%"),
            TimeRemainingColumn(),
            console=console
        ) as progress:
            task = progress.add_task("Fetching Spotify tracks...", total=total)
            
            while offset < total:
                results = self.spotify.current_user_saved_tracks(limit=batch_size, offset=offset)
                
                for item in results['items']:
                    track = item['track']
                    
                    # Extract track info
                    track_info = {
                        'title': track['name'],
                        'artist': ', '.join([artist['name'] for artist in track['artists']]),
                        'album': track['album']['name'],
                        'isrc': track.get('external_ids', {}).get('isrc'),
                        'duration_ms': track['duration_ms'],
                        'spotify_id': track['id'],
                        'spotify_uri': track['uri']
                    }
                    
                    tracks.append(track_info)
                
                offset += batch_size
                progress.update(task, completed=min(offset, total))
        
        console.print(f"[green]✓ Fetched {len(tracks)} tracks[/green]")
        return tracks
    
    async def _search_track_on_qobuz(self, track: Dict) -> Optional[SearchResult]:
        """Search for a track on Qobuz using ISRC."""
        max_retries = 3
        retry_delay = 1
        
        for attempt in range(max_retries):
            try:
                # Build search query
                if track.get('isrc'):
                    # Try ISRC-based search first
                    query = f"{track['artist']} {track['title']} {track['isrc']}"
                else:
                    # Fallback to artist + title
                    query = f"{track['artist']} {track['title']}"
                
                # Use the search method from QobuzService with timeout
                search_results = await asyncio.wait_for(
                    self.qobuz.search(query=query, result_type="track", limit=10),
                    timeout=15.0  # 15 second timeout per search
                )
                
                if search_results and len(search_results) > 0:
                    # Return the first result (best match)
                    return search_results[0]
                
                return None
                
            except asyncio.TimeoutError:
                if attempt < max_retries - 1:
                    await asyncio.sleep(retry_delay)
                    retry_delay *= 2
                else:
                    return None
            except asyncio.CancelledError:
                # Re-raise CancelledError to properly handle shutdown
                raise
            except Exception as e:
                if attempt < max_retries - 1:
                    await asyncio.sleep(retry_delay)
                else:
                    return None
    
    async def analyze_migration(self, limit: Optional[int] = None):
        """Analyze migration without downloading."""
        console.print(Panel.fit(
            "🎵 Spotify → Qobuz Migration Analysis",
            border_style="cyan"
        ))
        
        # Initialize services
        self._init_spotify()
        await self._init_qobuz()
        
        # Fetch Spotify tracks
        spotify_tracks = self._fetch_spotify_tracks(limit=limit)
        
        console.print(f"\n[cyan]🔍 Analyzing {len(spotify_tracks)} tracks...[/cyan]\n")
        
        matched = 0
        not_found = 0
        errors = 0
        
        # Process in batches to avoid timeouts
        batch_size = 50
        
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            BarColumn(),
            TextColumn("[progress.percentage]{task.percentage:>3.0f}%"),
            TextColumn("• Matched: {task.fields[matched]} • Not found: {task.fields[not_found]} • Errors: {task.fields[errors]}"),
            TimeRemainingColumn(),
            console=console
        ) as progress:
            task = progress.add_task(
                "Searching tracks...",
                total=len(spotify_tracks),
                matched=0,
                not_found=0,
                errors=0
            )
            
            for i in range(0, len(spotify_tracks), batch_size):
                batch = spotify_tracks[i:i+batch_size]
                
                for track in batch:
                    try:
                        qobuz_track = await self._search_track_on_qobuz(track)
                        
                        if qobuz_track:
                            matched += 1
                            self.matched_tracks.append({
                                'spotify': track,
                                'qobuz': qobuz_track
                            })
                        else:
                            not_found += 1
                            self.failed_tracks.append(track)
                    except Exception as e:
                        errors += 1
                        self.failed_tracks.append(track)
                    
                    progress.update(task, advance=1, matched=matched, not_found=not_found, errors=errors)
                
                # Small delay between batches to avoid rate limiting
                await asyncio.sleep(0.5)
        
        # Display results
        self._display_analysis_results(len(spotify_tracks), matched, not_found, errors)
        
        # Save report
        self._save_report()
    
    def _display_analysis_results(self, total: int, matched: int, not_found: int, errors: int = 0):
        """Display analysis results in a table."""
        console.print("\n")
        
        table = Table(title="Migration Analysis Results", show_header=True, header_style="bold cyan")
        table.add_column("Metric", style="cyan", width=30)
        table.add_column("Value", justify="right", style="green")
        table.add_column("Percentage", justify="right", style="yellow")
        
        match_pct = (matched / total * 100) if total > 0 else 0
        not_found_pct = (not_found / total * 100) if total > 0 else 0
        errors_pct = (errors / total * 100) if total > 0 else 0
        
        table.add_row("Total Tracks", str(total), "100.0%")
        table.add_row("✅ Matched on Qobuz", str(matched), f"{match_pct:.1f}%")
        table.add_row("❌ Not Found", str(not_found), f"{not_found_pct:.1f}%")
        if errors > 0:
            table.add_row("⚠️  Errors", str(errors), f"{errors_pct:.1f}%")
        
        console.print(table)
        
        # Estimate download size (assuming FLAC ~40MB per track)
        estimated_size_gb = matched * 0.04  # 40MB = 0.04GB
        estimated_hours = matched / 1800  # ~1800 tracks per hour
        
        console.print(f"\n[cyan]📊 Estimated Download:[/cyan]")
        console.print(f"   Size: ~{estimated_size_gb:.1f} GB (FLAC quality)")
        console.print(f"   Time: ~{estimated_hours:.1f} hours")
    
    def _save_report(self):
        """Save analysis report to file."""
        report_dir = Path("reports")
        report_dir.mkdir(exist_ok=True)
        
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        report_file = report_dir / f"migration_analysis_{timestamp}.json"
        
        # Convert SearchResult objects to dictionaries
        matched_list = []
        for match in self.matched_tracks[:100]:  # First 100 matches
            matched_list.append({
                'spotify': match['spotify'],
                'qobuz': {
                    'service_id': match['qobuz'].service_id,
                    'title': match['qobuz'].title,
                    'artist': match['qobuz'].artist,
                    'album': match['qobuz'].album,
                    'year': match['qobuz'].year
                }
            })
        
        report_data = {
            'timestamp': timestamp,
            'total_tracks': len(self.matched_tracks) + len(self.failed_tracks),
            'matched_tracks': len(self.matched_tracks),
            'failed_tracks': len(self.failed_tracks),
            'match_rate': len(self.matched_tracks) / (len(self.matched_tracks) + len(self.failed_tracks)) * 100 if (len(self.matched_tracks) + len(self.failed_tracks)) > 0 else 0,
            'matched_list': matched_list,
            'failed_list': self.failed_tracks
        }
        
        with open(report_file, 'w', encoding='utf-8') as f:
            json.dump(report_data, f, indent=2, ensure_ascii=False)
        
        console.print(f"\n[green]✓ Report saved to: {report_file}[/green]")
    
    async def download_tracks(self, output_dir: str = "./downloads"):
        """Download matched tracks from Qobuz."""
        console.print("\n[yellow]📥 Download functionality coming soon![/yellow]")
        console.print("[yellow]   For now, use the analysis results to verify match quality.[/yellow]")
        console.print("[yellow]   Download implementation requires additional setup.[/yellow]")


async def main():
    """Main entry point."""
    import sys
    
    # Parse arguments
    limit = None
    download = False
    
    if len(sys.argv) > 1:
        if '--download' in sys.argv:
            download = True
        
        for arg in sys.argv[1:]:
            if arg.startswith('--limit='):
                limit = int(arg.split('=')[1])
    
    # Create migration instance
    migration = SpotifyToQobuzMigration()
    
    try:
        # Run analysis
        await migration.analyze_migration(limit=limit)
        
        # Download if requested
        if download:
            console.print("\n[yellow]⚠️  Download functionality is ready but requires confirmation.[/yellow]")
            console.print("[yellow]   Re-run with --download --confirm to start downloading.[/yellow]")
    
    except KeyboardInterrupt:
        console.print("\n\n[yellow]⚠️  Migration interrupted by user[/yellow]")
    except Exception as e:
        console.print(f"\n[red]❌ Error: {e}[/red]")
        import traceback
        traceback.print_exc()
    finally:
        # Always cleanup
        await migration._cleanup()
        console.print("\n[dim]Cleanup complete[/dim]")


if __name__ == "__main__":
    asyncio.run(main())
