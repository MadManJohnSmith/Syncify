import sqlite3,time,statistics,os
sets={}
for kb in (2000,64000,262144):
 c=sqlite3.connect('file:/home/alan/.local/share/com.syncify.app/syncify.db?mode=ro',uri=True)
 c.execute(f'PRAGMA cache_size={-kb}')
 for label,sql in [('worker_select','''SELECT t.id,t.title,(SELECT a.name FROM track_artists ta JOIN artists a ON a.id=ta.artist_id WHERE ta.track_id=t.id LIMIT 1),(SELECT alb.title FROM albums alb WHERE alb.id=t.album_id),t.isrc FROM tracks t LEFT JOIN enrichment_progress ep ON ep.track_id=t.id AND ep.service='all' WHERE (ep.status IS NULL OR ep.status='pending' OR (ep.status='failed' AND ep.retry_count<3)) ORDER BY t.id ASC LIMIT 1'''),('enrichment_status','''SELECT (SELECT COUNT(*) FROM tracks t LEFT JOIN enrichment_progress ep ON ep.track_id=t.id WHERE ep.status IS NULL OR ep.status='pending'),(SELECT COUNT(*) FROM enrichment_progress WHERE status='completed'),(SELECT COUNT(*) FROM enrichment_progress WHERE status='failed')''')]:
  for warm in (False,True):
   ts=[]
   for i in range(12):
    st=time.perf_counter_ns(); c.execute(sql).fetchall(); ts.append((time.perf_counter_ns()-st)/1e6)
   ts.sort()
   key=(kb//1024,label,'warm' if warm else 'cold')
   sets[key]=(round(statistics.median(ts),3),round(ts[-1],3))
for k,v in sorted(sets.items()): print(k[0],'MiB',k[1],k[2],'median_ms',v[0],'max_ms',v[1])
