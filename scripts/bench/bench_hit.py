import sqlite3,sys,time,statistics
path=sys.argv[1]
c=sqlite3.connect(path,timeout=30)
read='''SELECT mbid,payload FROM mb_library_index i WHERE i.isrc=? AND i.updated_at>=datetime('now',?) AND i.updated_at<=datetime('now') AND EXISTS(SELECT 1 FROM tracks t WHERE UPPER(REPLACE(REPLACE(TRIM(t.isrc),'-',''),' ',''))=i.isrc)'''
values=['ZZZZZ%09d'%i for i in range(250)]
def bench():
 ts=[]
 for v in values:
  st=time.perf_counter_ns(); c.execute(read,(v,'-30 days')).fetchone(); ts.append((time.perf_counter_ns()-st)/1e6)
 ts.sort(); print(path.split('/')[-1],'L3_read_hit_probe n=250 median_ms',round(statistics.median(ts),4),'p95_ms',round(ts[237],4),'total_s',round(sum(ts)/1000,2))
bench()
