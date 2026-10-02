#!/usr/bin/env python3
"""Stream canonical JSONL market events into compressed, bounded Parquet batches."""
import argparse
import json
import time
from pathlib import Path
import pyarrow as pa
import pyarrow.parquet as pq

SCHEMA=pa.schema([('exchange',pa.string()),('symbol',pa.string()),('event_type',pa.string()),
                 ('exchange_ts',pa.uint64()),('receive_ts',pa.uint64()),('sequence_id',pa.int64()),('payload',pa.string())])

def export(source,output,watch=False):
    output=Path(output);output.mkdir(parents=True,exist_ok=True)
    counter=0;batch=[];last=time.monotonic();prefix=Path(source).stem
    # No overwrite: each recorder run owns its own directory.
    dest=output/f'{prefix}-{time.time_ns()}';dest.mkdir()
    def flush():
        nonlocal counter,batch,last
        if batch:
            tmp=dest/f'{counter:06d}.parquet.tmp';final=tmp.with_suffix('')
            pq.write_table(pa.Table.from_pylist(batch,schema=SCHEMA),tmp,compression='zstd')
            tmp.rename(final);counter+=1;batch=[];last=time.monotonic()
    try:
        with open(source) as f:
            while True:
                pos=f.tell();line=f.readline()
                if not line or not line.endswith('\n'):
                    if not watch:
                        if line: raise ValueError('truncated journal: last line is incomplete')
                        break
                    f.seek(pos)
                    if time.monotonic()-last>=5: flush()
                    time.sleep(.25);continue
                obj=json.loads(line)
                if obj['type']!='event':continue
                event=obj['event'];keys=('exchange','symbol','event_type','exchange_ts','receive_ts','sequence_id')
                batch.append({**{k:event[k] for k in keys},'payload':json.dumps({k:v for k,v in event.items() if k not in keys},separators=(',',':'))})
                if len(batch)>=10_000 or time.monotonic()-last>=5:flush()
    finally:flush()
    print(f'{counter} Parquet batches written to {dest}')

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('recording');parser.add_argument('--output',default='data/parquet');parser.add_argument('--watch',action='store_true')
    args=parser.parse_args()
    try:export(args.recording,args.output,args.watch)
    except KeyboardInterrupt:pass
