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

def metrics(rows, prob, eligible=None):
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
    if eligible is not None: selected &= eligible
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

def walk_forward(rows):
    """Expanding training, 10% calibration, disjoint forward test windows."""
    times = sorted({r['prediction']['ts'] for r in rows})
    if len(times) < 100:
        raise ValueError('滚动验证至少需要 100 个不同时间点，推荐数百条真实预测')
    edges = [int(len(times)*v) for v in (.6, .7333333333, .8666666667)] + [len(times)]
    folds = []
    for start, end in zip(edges, edges[1:]):
        cal_start = times[start - int(len(times)*.1)]
        test_start = times[start]
        test_end = times[end] if end < len(times) else float('inf')
        train = [r for r in rows if r['prediction']['ts'] < cal_start and r['first_hit_ts'] < cal_start]
        cal = [r for r in rows if cal_start <= r['prediction']['ts'] < test_start and r['first_hit_ts'] < test_start]
        test = [r for r in rows if test_start <= r['prediction']['ts'] < test_end]
        if min(map(len, (train, cal, test))) < 20:
            raise ValueError('清除跨界标签后，每轮训练 / 校准 / 验证至少需要 20 条样本')
        folds.append((train, cal, test))
    return folds


def distribution_profile(rows):
    profiles = {}
    # Floors are in the normalized Rust/Python feature-vector units, including constant features.
    floors = np.array([1., .05, .05, .05, .1, .1, .05, .1, .1, .05])
    for symbol in sorted({r['prediction']['symbol'] for r in rows}):
        selected = [r for r in rows if r['prediction']['symbol'] == symbol]
        x = np.array([vector(r['prediction']['features']) for r in selected])
        profiles[symbol] = dict(mean=x.mean(axis=0).tolist(), scale=np.maximum(x.std(axis=0), floors).tolist(),
                                max_z=6., samples=len(selected),
                                volatility_boundary=float(np.quantile([r['prediction']['features']['volatility'] for r in selected], .75)),
                                spread_boundary=float(np.quantile([r['prediction']['features']['spread_bps'] for r in selected], .75)))
    return profiles


def regime(row, profiles):
    p = row['prediction']; f = p['features']; d = profiles.get(p['symbol'])
    if d is None: return 'UNKNOWN'
    vol = f['volatility'] > d['volatility_boundary']; wide = f['spread_bps'] > d['spread_boundary']
    return p['symbol'] + ':' + ('STRESSED' if vol and wide else 'HIGH_VOL' if vol else 'WIDE_SPREAD' if wide else 'NORMAL')


def within_distribution(row, profiles):
    d = profiles.get(row['prediction']['symbol'])
    return d is not None and float(np.max(np.abs((np.array(vector(row['prediction']['features'])) - d['mean']) / d['scale'])[1:])) <= d['max_z']


def fit_regimes(rows, logits, labels, profiles):
    result = {}
    keys = np.array([regime(r, profiles) for r in rows])
    for key in sorted(set(keys)):
        mask = (keys == key) & np.array([within_distribution(r, profiles) for r in rows])
        if key != 'UNKNOWN' and mask.sum() >= 20:
            result[key] = dict(temperature=calibrate(logits[mask], labels[mask]), samples=int(mask.sum()))
    return result


def regime_probabilities(rows, logits, profiles, calibrations):
    # Missing regimes retain RAW probabilities for diagnostics and cannot be selected for trading.
    prob = softmax(logits)
    eligible = np.zeros(len(rows), dtype=bool)
    for i, row in enumerate(rows):
        calibration = calibrations.get(regime(row, profiles))
        if calibration:
            prob[i] = softmax(logits[i:i+1], calibration['temperature'])[0]
            eligible[i] = within_distribution(row, profiles)
    return prob, eligible


def fit_fold(train, cal, test):
    arrays = lambda rs: (np.array([vector(r['prediction']['features']) for r in rs]), np.array([target(r) for r in rs]))
    xt, yt = arrays(train); xc, yc = arrays(cal); xv, _ = arrays(test)
    weights = fit(xt, yt)
    profiles = distribution_profile(train)
    fitted = {}
    for name, cal_logits, test_logits in (
        ('score', np.array([score_logits(r['prediction']['features']) for r in cal]), np.array([score_logits(r['prediction']['features']) for r in test])),
        ('logistic', xc @ weights.T, xv @ weights.T)):
        calibrations = fit_regimes(cal, cal_logits, yc, profiles)
        probs, eligible = regime_probabilities(test, test_logits, profiles, calibrations)
        fitted[name] = dict(temperature=calibrate(cal_logits, yc), calibrations=calibrations,
                            probs=probs, eligible=eligible, metrics=metrics(test, probs, eligible))
    return weights, profiles, fitted


def run(args):
    with sqlite3.connect(f'file:{Path(args.db).resolve()}?mode=ro', uri=True) as db:
        records = [json.loads(row[0]) for row in db.execute("SELECT payload FROM records WHERE kind='outcome' ORDER BY ts,id")]
    records = [r for r in records if r['source'] == args.source and r['payload']['learning_eligible']]
    if not records: raise ValueError('没有匹配来源的有效自动 Outcome')
    rows = [r['payload'] for r in records]
    latest = max(rows, key=lambda o: o['prediction']['ts'])['prediction']
    definition = lambda p: (p['config_version'], p['horizon'], p['target'], p['stop'])
    rows = sorted({o['prediction']['prediction_id']: o for o in rows if definition(o['prediction']) == definition(latest)}.values(), key=lambda r: r['prediction']['ts'])
    if any(not np.isfinite(vector(o['prediction']['features'])).all() for o in rows):
        raise ValueError('特征必须为有限数值')
    if any(o['first_hit_ts'] < o['prediction']['ts'] for o in rows):
        raise ValueError('结果时间不能早于预测时间')
    windows = []; all_test = []; predictions = {'score': [], 'logistic': []}; eligibility = {'score': [], 'logistic': []}
    window = lambda rs: {'start': min(r['prediction']['ts'] for r in rs), 'end': max(r['first_hit_ts'] for r in rs), 'samples': len(rs)}
    for number, (train, cal, test) in enumerate(walk_forward(rows), 1):
        weights, profiles, fitted = fit_fold(train, cal, test)
        all_test.extend(test)
        for name in predictions:
            predictions[name].extend(fitted[name]['probs']); eligibility[name].extend(fitted[name]['eligible'])
        chosen = fitted[args.model]
        windows.append(dict(fold=number, train=window(train), calibration=window(cal), test=window(test),
                            training_digest=hashlib.sha256(json.dumps([r['prediction']['prediction_id'] for r in train]).encode()).hexdigest(),
                            calibration_coverage=float(chosen['eligible'].mean()),
                            score=fitted['score']['metrics'], logistic=fitted['logistic']['metrics'], **chosen['metrics']))
    reports = {name: metrics(all_test, np.array(predictions[name]), np.array(eligibility[name])) for name in predictions}
    chosen = reports[args.model]; eligible = np.array(eligibility[args.model])
    symbols = {s: sum(o['prediction']['symbol'] == s for o in all_test) for s in ('BTCUSDT', 'ETHUSDT')}
    # The final deployment artifact is exactly the last fold model, with its own untouched test evidence.
    final = fitted[args.model]
    regime_reports = {}
    final_keys = [regime(r, profiles) for r in test]
    for key in sorted(set(final_keys)):
        indices = np.array([i for i, k in enumerate(final_keys) if k == key and final['eligible'][i]], dtype=int)
        if len(indices): regime_reports[key] = metrics([test[i] for i in indices], final['probs'][indices])
    checks = {'enough_oos': len(all_test) >= 100, 'both_symbols': min(symbols.values()) >= 30,
              'calibration': chosen['ece'] <= .12, 'calibration_coverage': float(eligible.mean()) >= .9,
              'positive_observed_edge': chosen['selected'] >= 30 and chosen['observed_net_edge'] > 0,
              'window_stability': all(w['selected'] >= 5 and w['observed_net_edge'] > 0 and w['ece'] <= .12 and w['calibration_coverage'] >= .9 for w in windows),
              'regime_calibration': bool(regime_reports) and all(r['samples'] >= 20 and r['ece'] <= .12 for r in regime_reports.values()),
              'training_distribution': all(s in profiles and profiles[s]['samples'] >= 20 for s in symbols),
              'tail': chosen['tail_loss_p05'] >= -latest['stop']*3,
              'baseline_comparison': args.model == 'score' or all(w['logistic']['ece'] <= w['score']['ece'] and w['logistic']['observed_net_edge'] >= w['score']['observed_net_edge'] for w in windows)}
    stamp = str(latest['ts'])
    artifact = {'model_version': f'{args.model}-{args.source}-{stamp}', 'model_type': 'logistic-regression' if args.model == 'logistic' else 'deterministic-score',
                'feature_schema_version': 'micro-v1', 'label_version': 'first-hit-v1', 'calibration_version': f'regime-temperature-{stamp}',
                'artifact_hash': 'computed-by-engine-on-load', 'training_window': json.dumps(window(train)), 'validation_window': json.dumps(window(test)),
                'source': args.source, 'temperature': final['temperature'], 'weights': weights.tolist() if args.model == 'logistic' else None,
                'validated': all(checks.values()), 'horizon': latest['horizon'], 'target': latest['target'], 'stop': latest['stop'],
                'cost_bps': (latest['qualification']['costs']['total'] - latest['qualification']['costs']['spread'])*10000,
                'distributions': profiles, 'regime_calibrations': final['calibrations']}
    out = Path(args.output); out.mkdir(parents=True, exist_ok=True)
    encoded = json.dumps(artifact, indent=2, allow_nan=False).encode(); (out/'challenger.json').write_bytes(encoded)
    report = {'status': 'PAPER_RESEARCH_GATE_PASS' if all(checks.values()) else 'REJECTED', 'source': args.source,
              'counts': {'train': len(train), 'calibration': len(cal), 'test': len(all_test), 'symbols': symbols}, 'checks': checks,
              'score_baseline': reports['score'], 'logistic': reports['logistic'], 'chronological_test_windows': windows,
              'walk_forward': {'folds': len(windows), 'refit_each_fold': True, 'purged_boundaries': True, 'test_windows_disjoint': True},
              'final_model_regimes': regime_reports, 'sha256': hashlib.sha256(encoded).hexdigest(),
              'limitations': ['No live promotion. No shadow execution evidence.',
                              'OOD uses per-symbol marginal z-scores, not a joint-distribution drift test.',
                              'Sparse regimes retain raw diagnostic probabilities and are excluded from trade selection.',
                              'Observed net edge uses mid-price labels less configured costs; paper fills are separate.',
                              'Drawdown is a unit-notional research statistic, not portfolio drawdown.']}
    (out/'report.json').write_text(json.dumps(report, indent=2, allow_nan=False))
    print(json.dumps(report, indent=2))

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--db',required=True);parser.add_argument('--source',choices=['live','demo'],required=True)
    parser.add_argument('--model',choices=['score','logistic'],default='score');parser.add_argument('--output',default='data/research')
    try: run(parser.parse_args())
    except (ValueError,KeyError) as e: parser.exit(2,f'Research rejected: {e}\n')
