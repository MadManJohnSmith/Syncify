import sqlite3
import time
import sys

DB_PATH = '/home/alan/.local/share/com.syncify.app/syncify.db'

print("[wait_for_downloads] Iniciando monitor de cola de descargas...", flush=True)

last_completed = -1
idle_cycles = 0

while True:
    try:
        con = sqlite3.connect(f'file:{DB_PATH}?mode=ro', uri=True)
        cur = con.cursor()
        
        counts = dict(cur.execute('SELECT status, COUNT(*) FROM download_queue GROUP BY status').fetchall())
        con.close()
        
        complete = counts.get('complete', 0)
        downloading = counts.get('downloading', 0)
        queued = counts.get('queued', 0)
        failed = counts.get('failed', 0)
        total = complete + downloading + queued + failed
        
        remaining = downloading + queued
        
        if complete != last_completed:
            print(f"[Progreso] Completadas: {complete}/{total} | Descargando: {downloading} | En cola: {queued} | Fallidas: {failed} (Restantes: {remaining})", flush=True)
            last_completed = complete
            idle_cycles = 0
        else:
            idle_cycles += 1
            
        if remaining == 0:
            print(f"[wait_for_downloads] ¡Cola completada! Todas las descargas activas han finalizado (Completadas: {complete}, Fallidas: {failed}).", flush=True)
            break
            
        # Si no hay descargas activas ni en cola, o si lleva 15 minutos sin cambios y downloading es 0
        if downloading == 0 and queued == 0:
            print("[wait_for_downloads] No hay tareas en curso ni en cola.", flush=True)
            break
            
        # Intervalo de chequeo
        time.sleep(10)
        
    except Exception as e:
        print(f"[Error en monitor] {e}", flush=True)
        time.sleep(10)

print("[wait_for_downloads] Tarea finalizada con éxito.", flush=True)
