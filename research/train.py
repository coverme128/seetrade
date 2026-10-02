#!/usr/bin/env python3
"""Offline, chronological score/logistic research. Never edits production artifacts."""
import argparse
import hashlib
import json
import math
import sqlite3
from pathlib import Path
import numpy as np

def vector(f):
    return [1., f['book_imbalance'], f['flow_imbalance'], f['aggressive_imbalance'],
            f['microprice_bps']/5, f['momentum']*1000, f['volatility']*1000,
            (f['cross_venue_bps'] or 0)/10, f['spread_bps']/5, math.log(max(f['depth'], 1))/15]

def score_logits(f):
    s = np.clip(1.5*f['book_imbalance'] + .8*f['flow_imbalance'] + 1.2*f['aggressive_imbalance']
                + .15*f['microprice_bps'] + np.clip(f['momentum']*500, -1, 1), -3, 3)
    return [s, -s, -.2]

def softmax(logits, temperature=1.):
    z = np.asarray(logits, dtype=float) / temperature
    z -= z.max(axis=1, keepdims=True)
    p = np.exp(z)
    return p / p.sum(axis=1, keepdims=True)

def target(o):
    long = o['prediction']['direction'] == 'LONG'
    if o['outcome_type'] == 'TARGET_HIT':
        return 0 if long else 1
    if o['outcome_type'] in ('STOP_HIT', 'STRONG_OPPOSITE_MOVE'):
        return 1 if long else 0
    return 2

def fit(x, y, steps=300):
    w = np.zeros((3, x.shape[1]))
    one_hot = np.eye(3)[y]
    for _ in range(steps):
        prob = softmax(x @ w.T)
        gradient = (prob-one_hot).T @ x / len(x)
        penalty = .002*w
        penalty[:, 0] = 0
        w -= .08*(gradient+penalty)
    return w

def calibrate(logits, labels):
    return float(min(np.linspace(.2, 5, 97), key=lambda t: -np.log(np.maximum(softmax(logits,t)[np.arange(len(labels)),labels],1e-12)).mean()))

def metrics(rows, prob):
    labels = np.array([target(o) for o in rows])
    wins = np.array([o['outcome_type']=='TARGET_HIT' for o in rows], dtype=float)
    direction = np.array([0 if o['prediction']['direction']=='LONG' else 1 for o in rows])
    pwin = prob[np.arange(len(rows)), direction]
    ploss = prob[np.arange(len(rows)), 1-direction]
    ece = 0.
    for i in range(10):
        mask = np.minimum((pwin*10).astype(int),9)==i
        if mask.any(): ece += abs(pwin[mask].mean()-wins[mask].mean())*mask.mean()
    costs = np.array([o['prediction']['qualification']['costs']['total'] for o in rows])
    expected = pwin*np.array([o['prediction']['target'] for o in rows])-ploss*np.array([o['prediction']['stop'] for o in rows])-costs
    selected = expected > .0002
    observed = np.array([o['observed_net_edge'] for o in rows])
    edge = observed[selected]
    equity = np.r_[0, np.cumsum(edge)]
    dd = float(np.max(np.maximum.accumulate(equity)-equity))
    return {'samples':len(rows), 'selected':int(selected.sum()), 'ece':float(ece),
            'brier':float(np.mean(np.sum((prob-np.eye(3)[labels])**2,axis=1))),
            'log_loss':float(-np.log(np.maximum(prob[np.arange(len(labels)),labels],1e-12)).mean()),
            'expected_net_edge':float(expected[selected].mean()) if selected.any() else 0.,
            'observed_net_edge':float(edge.mean()) if len(edge) else 0.,
            'unit_notional_drawdown':dd, 'tail_loss_p05':float(np.quantile(edge,.05)) if len(edge) else 0.,
            'strong_opposite_rate':sum(o['outcome_type']=='STRONG_OPPOSITE_MOVE' for o in rows)/len(rows)}

def split_rows(rows):
    times = sorted({r['prediction']['ts'] for r in rows})
    if len(times)<30: raise ValueError('至少需要 30 个不同时间点，推荐数百条真实预测')
    a,b = times[int(len(times)*.5)], times[int(len(times)*.7)]
    # Purge labels crossing the boundary; no outcome from the next window leaks backwards.
    train=[r for r in rows if r['prediction']['ts']<a and r['first_hit_ts']<a]
    cal=[r for r in rows if a<=r['prediction']['ts']<b and r['first_hit_ts']<b]
    test=[r for r in rows if r['prediction']['ts']>=b]
    if min(map(len,(train,cal,test)))<20: raise ValueError('时间隔离后每个窗口至少需要 20 个样本')
    return train,cal,test

def run(args):
    with sqlite3.connect(f'file:{Path(args.db).resolve()}?mode=ro',uri=True) as db:
        records=[json.loads(row[0]) for row in db.execute("SELECT payload FROM records WHERE kind='outcome' ORDER BY ts,id")]
    records=[r for r in records if r['source']==args.source and r['payload']['learning_eligible']]
    if not records: raise ValueError('没有匹配来源的有效自动 Outcome')
    rows=[r['payload'] for r in records]
    latest=rows[-1]['prediction']
    rows=[o for o in rows if (o['prediction']['config_version'],o['prediction']['horizon'],o['prediction']['target'],o['prediction']['stop']) ==
          (latest['config_version'],latest['horizon'],latest['target'],latest['stop'])]
    # Deduplicate by prediction identity, then chronological order.
    rows=sorted({o['prediction']['prediction_id']:o for o in rows}.values(),key=lambda r:r['prediction']['ts'])
    train,cal,test=split_rows(rows)
    arrays=lambda rs:(np.array([vector(r['prediction']['features']) for r in rs]),np.array([target(r) for r in rs]))
    xt,yt=arrays(train);xc,yc=arrays(cal);xv,yv=arrays(test)
    weights=fit(xt,yt)
    score_cal=np.array([score_logits(o['prediction']['features']) for o in cal]);score_test=np.array([score_logits(o['prediction']['features']) for o in test])
    score_temp=calibrate(score_cal,yc);logistic_temp=calibrate(xc@weights.T,yc)
    score_report=metrics(test,softmax(score_test,score_temp));logistic_report=metrics(test,softmax(xv@weights.T,logistic_temp))
    chosen=logistic_report if args.model=='logistic' else score_report
    oos_probs=softmax(xv@weights.T,logistic_temp) if args.model=='logistic' else softmax(score_test,score_temp)
    windows=[]
    for indices in np.array_split(np.arange(len(test)),3):
        windows.append(metrics([test[i] for i in indices],oos_probs[indices]))
    symbols={s:sum(o['prediction']['symbol']==s for o in test) for s in ('BTCUSDT','ETHUSDT')}
    checks={'enough_oos':len(test)>=100,'both_symbols':min(symbols.values())>=30,'calibration':chosen['ece']<=.12,
            'positive_observed_edge':chosen['selected']>=30 and chosen['observed_net_edge']>0,
            'window_stability':all(w['selected']>=5 and w['observed_net_edge']>0 for w in windows),
            'tail':chosen['tail_loss_p05']>=-latest['stop']*3,
            'baseline_comparison':args.model=='score' or (chosen['ece']<=score_report['ece'] and chosen['observed_net_edge']>=score_report['observed_net_edge'])}
    stamp=str(latest['ts']);window=lambda rs:f"{rs[0]['prediction']['ts']}..{rs[-1]['first_hit_ts']}"
    artifact={'model_version':f'{args.model}-{args.source}-{stamp}','model_type':'logistic-regression' if args.model=='logistic' else 'deterministic-score',
              'feature_schema_version':'micro-v1','label_version':'first-hit-v1','calibration_version':f'temperature-{stamp}',
              'artifact_hash':'computed-by-engine-on-load','training_window':window(train),'validation_window':window(test),
              'source':args.source,'temperature':logistic_temp if args.model=='logistic' else score_temp,
              'weights':weights.tolist() if args.model=='logistic' else None,'validated':all(checks.values()),
              'horizon':latest['horizon'],'target':latest['target'],'stop':latest['stop'],
              'cost_bps':(latest['qualification']['costs']['total']-latest['qualification']['costs']['spread'])*10000}
    out=Path(args.output);out.mkdir(parents=True,exist_ok=True)
    encoded=json.dumps(artifact,indent=2,allow_nan=False).encode();(out/'challenger.json').write_bytes(encoded)
    report={'status':'PAPER_RESEARCH_GATE_PASS' if all(checks.values()) else 'REJECTED','source':args.source,
            'counts':{'train':len(train),'calibration':len(cal),'test':len(test),'symbols':symbols},'checks':checks,
            'score_baseline':score_report,'logistic':logistic_report,'chronological_test_windows':windows,
            'sha256':hashlib.sha256(encoded).hexdigest(),
            'limitations':['No live promotion. No shadow execution evidence.','Observed net edge uses mid-price labels less configured costs; paper fills are separate.','Drawdown is a unit-notional research statistic, not portfolio drawdown.']}
    (out/'report.json').write_text(json.dumps(report,indent=2,allow_nan=False))
    print(json.dumps(report,indent=2))

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--db',required=True);parser.add_argument('--source',choices=['live','demo'],required=True)
    parser.add_argument('--model',choices=['score','logistic'],default='score');parser.add_argument('--output',default='data/research')
    try: run(parser.parse_args())
    except (ValueError,KeyError) as e: parser.exit(2,f'Research rejected: {e}\n')
