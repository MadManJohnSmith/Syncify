import asyncio
import os
import sys
from dotenv import load_dotenv

load_dotenv()  # Cargar variables de entorno desde .env

# Añadir el directorio del proyecto al sys.path para asegurar que spotify_sync_lib sea importable
# Esto es útil si ejecutas main.py directamente y spotify_sync_lib no está instalado como un paquete.
SCRIPT_DIR_MAIN = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, SCRIPT_DIR_MAIN) # Permite importar spotify_sync_lib

from spotify_sync_lib.app_orchestrator import run_sync_process
from spotify_sync_lib.config import console # Importar solo console

if __name__ == "__main__":
    try:
        asyncio.run(run_sync_process(SCRIPT_DIR_MAIN)) # Pasar el project_root
    except KeyboardInterrupt:
        console.print("\n[yellow]Script interrumpido por el usuario.[/yellow]")
    except Exception as e:
        console.print(f"[bold red]Ocurrió un error crítico inesperado en main.py:[/bold red]\n{e}")
        console.print("Revisa el archivo de log para más detalles si fue creado.")
        console.print_exception(show_locals=True)