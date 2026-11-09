import sqlite3,os,time,statistics
source='file:/home/alan/.local/share/com.syncify.app/syncify.db?mode=ro'
q="CREATE INDEX probe_idx_norm_isrc ON tracks(UPPER(REPLACE(REPLACE(TRIM(isrc), '-', ''), ' ', '')))"
def run(lane,round_no):
 path=f'/home/alan/perf-probe/maintenance-{lane}-{round_no}.db'
 src=sqlite3.connect(source,uri=True,timeout=30)
 dst=sqlite3.connect(path,timeout=30)
 src.backup(dst);src.close()
 dst.execute('PRAGMA journal_mode=WAL')
 if lane=='indexed':
  dst.execute(q);dst.commit()
 ids=[r[0] for r in dst.execute("SELECT id FROM tracks WHERE isrc IS NOT NULL AND isrc<>'' LIMIT 250")]
 times=[]
 for i,tid in enumerate(ids):
  new_isrc=f'ZZZAA{i+round_no*1000:07d}'
  st=time.perf_counter_ns()
  dst.execute('UPDATE tracks SET isrc=? WHERE id=?',(new_isrc,tid));dst.commit()
  times.append((time.perf_counter_ns()-st)/1e6)
 times.sort()
 print(lane,round_no,'median_ms',round(statistics.median(times),3),'p95_ms',round(times[237],3),'total_ms',round(sum(times),1),flush=True)
 dst.close()
for r in range(2):
 lanes=['indexed','plain'] if r%2==0 else ['plain','indexed']
 for lane in lanes:run(lane,r)
