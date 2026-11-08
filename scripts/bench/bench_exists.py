import sqlite3,sys,time,statistics
c=sqlite3.connect(sys.argv[1],timeout=30)
# Mide solo el EXISTS, es el sitio exacto donde se decide el escaneo.
q='''SELECT 1 FROM tracks t WHERE UPPER(REPLACE(REPLACE(TRIM(t.isrc), '-', ''), ' ', '')) = ? LIMIT 1'''
hits=[r[0] for r in c.execute("SELECT isrc FROM tracks WHERE isrc IS NOT NULL AND isrc<>'' LIMIT 250")]
misses=['ZZZZZ%09d'%i for i in range(250)]
for name,vals in [('hit',hits),('miss',misses)]:
 ts=[]
 for v in vals:
  st=time.perf_counter_ns(); c.execute(q,(v,)).fetchone(); ts.append((time.perf_counter_ns()-st)/1e6)
 ts.sort()
 print(sys.argv[1].split('/')[-1],'EXISTS_'+name,'median_ms',round(statistics.median(ts),4),'p95_ms',round(ts[237],4),'total_s',round(sum(ts)/1000,3))
