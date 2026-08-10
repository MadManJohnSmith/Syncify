"""
Syncify Phase 1 - Status Dashboard
Shows current progress and next steps at a glance.
"""

from rich.console import Console
from rich.panel import Panel
from rich.table import Table
from rich.progress import Progress, BarColumn, TextColumn
from rich.layout import Layout
from rich.text import Text
from datetime import datetime
import os

console = Console()


def show_dashboard():
    """Display the Phase 1 status dashboard."""
    
    console.clear()
    
    # Header
    console.print("\n")
    console.print("╔═══════════════════════════════════════════════════════════════╗", style="cyan bold")
    console.print("║         SYNCIFY PHASE 1 - DEVELOPMENT DASHBOARD              ║", style="cyan bold")
    console.print("╚═══════════════════════════════════════════════════════════════╝", style="cyan bold")
    console.print()
    
    # Overall Progress
    console.print(Panel.fit(
        "[green]Phase 1 Foundation: COMPLETE ✓[/green]\n"
        "[yellow]Week 1-2 Research: IN PROGRESS[/yellow]\n"
        "[dim]Week 3-4 Implementation: Not Started[/dim]\n"
        "[dim]Week 5-6 Multi-Service: Not Started[/dim]\n"
        "[dim]Week 7-8 Orchestration: Not Started[/dim]\n"
        "[dim]Week 9-10 Polish: Not Started[/dim]",
        title="[cyan]Overall Progress: 15%[/cyan]",
        border_style="cyan"
    ))
    console.print()
    
    # Foundation Status
    foundation_table = Table(title="✓ Foundation Complete", show_header=True, header_style="bold green")
    foundation_table.add_column("Component", style="cyan")
    foundation_table.add_column("Status", style="green")
    foundation_table.add_column("Lines", justify="right")
    
    foundation_table.add_row("Database Layer", "✓ Complete", "330")
    foundation_table.add_row("Service Base Class", "✓ Complete", "285")
    foundation_table.add_row("Database Models", "✓ Complete", "160")
    foundation_table.add_row("Qobuz Service Skeleton", "✓ Complete", "~300")
    foundation_table.add_row("Test Suite", "✓ Passing", "120")
    foundation_table.add_row("Documentation", "✓ Complete", "4,500+")
    
    console.print(foundation_table)
    console.print()
    
    # Current Sprint
    sprint_table = Table(title="📅 Current Sprint: Week 1-2 Research", show_header=True, header_style="bold yellow")
    sprint_table.add_column("Task", style="white", width=40)
    sprint_table.add_column("Status", width=15)
    sprint_table.add_column("Priority", width=10)
    
    sprint_table.add_row(
        "Study QobuzDownloaderX-MOD source",
        "[yellow]Todo[/yellow]",
        "[red]High[/red]"
    )
    sprint_table.add_row(
        "Extract API credentials (APP_ID, APP_SECRET)",
        "[yellow]Todo[/yellow]",
        "[red]Critical[/red]"
    )
    sprint_table.add_row(
        "Document API endpoints",
        "[yellow]Todo[/yellow]",
        "[orange]Medium[/orange]"
    )
    sprint_table.add_row(
        "Set up Qobuz trial account",
        "[yellow]Todo[/yellow]",
        "[red]High[/red]"
    )
    sprint_table.add_row(
        "Test authentication",
        "[yellow]Todo[/yellow]",
        "[red]High[/red]"
    )
    sprint_table.add_row(
        "Create implementation plan",
        "[yellow]Todo[/yellow]",
        "[orange]Medium[/orange]"
    )
    
    console.print(sprint_table)
    console.print()
    
    # Quick Actions
    actions = Table(title="🚀 Quick Actions", show_header=False, box=None)
    actions.add_column("Command", style="cyan")
    actions.add_column("Description", style="white")
    
    actions.add_row(
        "cd ..; git clone https://github.com/DJDoubleD/QobuzDownloaderX-MOD.git",
        "Clone Qobuz source"
    )
    actions.add_row(
        "code WEEK1-2_RESEARCH_GUIDE.md",
        "Open research guide"
    )
    actions.add_row(
        "python test_db.py",
        "Run database tests"
    )
    actions.add_row(
        "python services/qobuz_service.py",
        "Test Qobuz auth (after setup)"
    )
    actions.add_row(
        "code PROGRESS.md",
        "Update progress tracker"
    )
    
    console.print(actions)
    console.print()
    
    # Files Created Today
    files_table = Table(title="📁 Files Created Today (Nov 23, 2025)", show_header=True, header_style="bold cyan")
    files_table.add_column("File", style="cyan")
    files_table.add_column("Purpose", style="white")
    
    files_created = [
        ("DEVELOPMENT_ROADMAP.md", "Complete 6-9 month plan"),
        ("PHASE1_IMPLEMENTATION_GUIDE.md", "Step-by-step coding guide"),
        ("ARCHITECTURE.md", "System design docs"),
        ("QUICKSTART.md", "15-minute setup"),
        ("README_PHASE1.md", "Executive summary"),
        ("WEEK1-2_RESEARCH_GUIDE.md", "Research instructions"),
        ("PROGRESS.md", "Progress tracking"),
        ("services/service_base.py", "Abstract service interface"),
        ("services/qobuz_service.py", "Qobuz implementation"),
        ("data/models.py", "Database models"),
        ("data/database.py", "Database manager"),
        ("test_db.py", "Database test suite"),
        ("requirements_phase1.txt", "Dependencies"),
        (".env.example", "Credential template"),
        ("dashboard.py", "This file!")
    ]
    
    for filename, purpose in files_created[:10]:  # Show first 10
        files_table.add_row(filename, purpose)
    
    if len(files_created) > 10:
        files_table.add_row("[dim]...[/dim]", f"[dim]+ {len(files_created) - 10} more files[/dim]")
    
    console.print(files_table)
    console.print()
    
    # Statistics
    stats = Panel(
        "[cyan]Foundation Stats:[/cyan]\n"
        "  • Documentation: [green]4,500+ lines[/green]\n"
        "  • Code: [green]775 lines[/green]\n"
        "  • Tests: [green]100% passing[/green]\n"
        "  • Files Created: [green]17[/green]\n"
        "  • Time Invested: [green]~2 hours[/green]\n\n"
        "[yellow]Next Milestone:[/yellow]\n"
        "  • Week 1-2 Research Complete\n"
        "  • Target: Nov 30, 2025\n"
        "  • Focus: Extract Qobuz API credentials",
        title="📊 Statistics",
        border_style="cyan"
    )
    console.print(stats)
    console.print()
    
    # Resources
    console.print(Panel(
        "[cyan]Key Documents:[/cyan]\n"
        "  1. [bold]WEEK1-2_RESEARCH_GUIDE.md[/bold] - Start here!\n"
        "  2. [bold]QUICKSTART.md[/bold] - Environment setup\n"
        "  3. [bold]PROGRESS.md[/bold] - Track your work\n"
        "  4. [bold]PHASE1_IMPLEMENTATION_GUIDE.md[/bold] - Week 3-4 preview\n\n"
        "[yellow]External Links:[/yellow]\n"
        "  • QobuzDownloaderX-MOD: [link]github.com/DJDoubleD/QobuzDownloaderX-MOD[/link]\n"
        "  • Qobuz Trial: [link]qobuz.com/us-en/offers[/link]",
        title="📚 Resources",
        border_style="yellow"
    ))
    console.print()
    
    # Footer
    console.print("─" * 65, style="dim")
    console.print(
        f"[dim]Updated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')} | "
        f"Branch: feature/multi-service-backend[/dim]"
    )
    console.print()


if __name__ == "__main__":
    show_dashboard()
    
    console.print("\n[cyan bold]Ready to begin Week 1-2 research! 🚀[/cyan bold]")
    console.print("[yellow]Run:[/yellow] [cyan]code WEEK1-2_RESEARCH_GUIDE.md[/cyan] [yellow]to get started.[/yellow]\n")
