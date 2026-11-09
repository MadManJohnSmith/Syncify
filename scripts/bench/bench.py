import sqlite3,sys,time,statistics
path=sys.argv[1]
c=sqlite3.connect(path,timeout=30)
c.execute("CREATE TABLE IF NOT EXISTS mb_library_index (isrc TEXT PRIMARY KEY, mbid TEXT NOT NULL, payload TEXT NOT NULL CHECK(json_valid(payload)), updated_at TEXT NOT NULL DEFAULT (datetime('now')))")
payload=c.execute("SELECT payload FROM mb_query_cache WHERE payload IS NOT NULL LIMIT 1").fetchone()[0]
mbid=c.execute("SELECT payload FROM mb_library_index LIMIT 1").fetchone()[0]
q='''INSERT INTO mb_library_index (isrc,mbid,payload) SELECT ?,?,? WHERE EXISTS(
 SELECT 1 FROM tracks t WHERE UPPER(REPLACE(REPLACE(TRIM(t.isrc), '-', ''), ' ', '')) = ?)
 ON CONFLICT(isrc) DO UPDATE SET mbid=excluded.mbid,payload=excluded.payload,updated_at=datetime('now')'''
read='''SELECT mbid,payload FROM mb_library_index i WHERE i.isrc=? AND i.updated_at>=datetime('now',?) AND i.updated_at<=datetime('now') AND EXISTS(SELECT 1 FROM tracks t WHERE UPPER(REPLACE(REPLACE(TRIM(t.isrc),'-',''),' ',''))=i.isrc)'''
def bench(fn,n,label):
 ts=[]
 for i in range(n):
  st=time.perf_counter_ns(); fn(i); ts.append((time.perf_counter_ns()-st)/1e6)
 ts.sort()
 print(path.split('/')[-1],label,'n',n,'median_ms',round(statistics.median(ts),4),'p95_ms',round(ts[int(n*.95)],4),'total_s',round(sum(ts)/1000,2))
bench(lambda i:(c.execute(q,(f'ZZZZZ{i:09d}',mbid,payload,f'ZZZZZ{i:09d}')),c.commit()),250,'L3_write_miss')
bench(lambda i:c.execute(read,(f'ZZZZZ{i:09d}','-30 days')).fetchone(),250,'L3_read_miss')
