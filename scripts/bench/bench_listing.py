import sqlite3,time,statistics
c=sqlite3.connect('file:/home/alan/.local/share/com.syncify.app/syncify.db?mode=ro',uri=True)
def bench(label,sql,args,n=5):
 times=[]
 for i in range(n):
  st=time.perf_counter_ns(); c.execute(sql,args).fetchall(); times.append((time.perf_counter_ns()-st)/1e6)
 times.sort()
 print(label,'median_ms',round(statistics.median(times),3),'max_ms',round(times[-1],3),'rows',len(c.execute(sql,args).fetchall()))
# 1) get_library como se escribio: subconsultas + LEFT JOIN + GROUP_CONCAT sobre 100 pistas
bench('get_library_page','''SELECT t.id,t.title,
 (SELECT a2.name FROM track_artists ta2 JOIN artists a2 ON a2.id=ta2.artist_id WHERE ta2.track_id=t.id AND ta2.role='primary' LIMIT 1) as artist_name,
 al.title as album_name,t.duration_ms,t.isrc,
 (SELECT GROUP_CONCAT(DISTINCT s_imp.name) FROM library_entries le JOIN accounts acc ON acc.id=le.account_id JOIN services s_imp ON s_imp.id=acc.service_id WHERE le.track_id=t.id) as imported_from
 FROM tracks t LEFT JOIN albums al ON al.id=t.album_id
 ORDER BY t.title ASC LIMIT 100''',())
# 2) misma pagina sin subconsultas de pertenencia
bench('get_library_join_only','''SELECT t.id,t.title,al.title as album_name,t.duration_ms,t.isrc
 FROM tracks t LEFT JOIN albums al ON al.id=t.album_id ORDER BY t.title ASC LIMIT 100''',())
# 3) SELECT del worker
bench('worker_select','''SELECT t.id,t.title,
 (SELECT a.name FROM track_artists ta JOIN artists a ON a.id=ta.artist_id WHERE ta.track_id=t.id LIMIT 1),
 (SELECT alb.title FROM albums alb WHERE alb.id=t.album_id), t.isrc
 FROM tracks t LEFT JOIN enrichment_progress ep ON ep.track_id=t.id AND ep.service='all'
 WHERE (ep.status IS NULL OR ep.status='pending' OR (ep.status='failed' AND ep.retry_count<3))
 ORDER BY t.id ASC LIMIT 1''',())
# 4) contadores de queue stats
bench('queue_stats','''SELECT
 (SELECT COUNT(*) FROM download_queue WHERE status='queued'),
 (SELECT COUNT(*) FROM download_queue WHERE status='downloading'),
 (SELECT COUNT(*) FROM download_queue WHERE status='complete'),
 (SELECT COUNT(*) FROM download_queue WHERE status='failed'),
 (SELECT COUNT(*) FROM download_queue WHERE status='cancelled'),
 (SELECT COALESCE(SUM(total_bytes),0) FROM download_queue WHERE status='complete')''',())
# 5) get_status de enrichment
bench('enrichment_status','''SELECT
 (SELECT COUNT(*) FROM tracks t LEFT JOIN enrichment_progress ep ON ep.track_id=t.id WHERE ep.status IS NULL OR ep.status='pending'),
 (SELECT COUNT(*) FROM enrichment_progress WHERE status='completed'),
 (SELECT COUNT(*) FROM enrichment_progress WHERE status='failed')''',())
