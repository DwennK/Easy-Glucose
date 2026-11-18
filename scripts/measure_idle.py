"""Read-only macOS/Linux process sampling. Keep the app untouched during the sample."""
import argparse
import json
import subprocess
import time
from pathlib import Path
p=argparse.ArgumentParser()
p.add_argument('pid',type=int)
p.add_argument('--seconds',type=int,default=30)
p.add_argument('--output',default='qa/idle.json')
a=p.parse_args()

def snapshot():
    values=subprocess.check_output(['ps','-p',str(a.pid),'-o','%cpu=,rss=,time='],text=True).strip().split()
    if len(values)!=3: raise RuntimeError('Process no longer running')
    parts=values[2].split(':')
    cpu_time=sum(float(value)*60**i for i,value in enumerate(reversed(parts)))
    return {'cpu_percent_snapshot':float(values[0]),'rss_kib':int(values[1]),'cpu_seconds':cpu_time}
start=snapshot()
begin=time.monotonic()
time.sleep(a.seconds)
end=snapshot()
elapsed=time.monotonic()-begin
result={'pid':a.pid,'elapsed_seconds':elapsed,'start':start,'end':end,'cpu_percent_from_process_time':100*(end['cpu_seconds']-start['cpu_seconds'])/elapsed,'rss_mib':end['rss_kib']/1024,'notes':'One local sample, not a performance guarantee; record visibility, mode, hardware and OS separately.'}
Path(a.output).parent.mkdir(exist_ok=True,parents=True)
Path(a.output).write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
