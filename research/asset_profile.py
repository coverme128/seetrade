"""Per-asset descriptive research artifacts. Never rewrites labels or promotes a model."""
import hashlib
import json
from pathlib import Path
import numpy as np

def write_profile(args, rows, evidence):
    symbol = getattr(args, 'symbol', None)
    if not symbol:
        return
    rows = [r for r in rows if r['prediction']['symbol'] == symbol]
    groups = {}
    for row in rows:
        p = row['prediction']
        key = f"{p['horizon']}:{p['target']}:{p['stop']}"
        groups.setdefault(key, []).append(row)
    cost_keys = ['spread', 'entry_fee', 'exit_fee', 'slippage', 'impact', 'adverse_selection', 'funding', 'total']
    costs = {key: {'median': float(np.median([r['prediction']['qualification']['costs'][key] for r in rows])),
                   'p95': float(np.quantile([r['prediction']['qualification']['costs'][key] for r in rows], .95))} for key in cost_keys} if rows else {}
    report = {
        'asset_key': symbol, 'source': args.source, 'schema': 'asset-research-v1',
        'dataset': evidence, 'samples': len(rows),
        'candidate_horizon_target_stop_evidence': [
            {'definition': dict(zip(['horizon', 'target', 'stop'], [g[0]['prediction'][k] for k in ['horizon', 'target', 'stop']])),
             'samples': len(g), 'target_hits': sum(r['outcome_type']=='TARGET_HIT' for r in g),
             'observed_net_edge': float(np.mean([r['observed_net_edge'] for r in g])),
             'mae_p95': float(np.quantile([r['mae'] for r in g], .95))} for _,g in sorted(groups.items())],
        'cost_profile': {'venue': 'recorded execution-venue assumptions; binance when no execution venue configured', 'estimates': costs, 'assumptions_not_confirmed_fills': True},
        'risk_profile': {'tail_move_p05': float(np.quantile([r['final_move'] for r in rows], .05)) if rows else None},
        'limitations': ['Only actually observed H/T/S definitions are reported; unobserved grids require replay and new chronological validation.',
                        'Cost components are recorded assumptions, not verified exchange fills.',
                        'No automatic configuration, lifecycle or model promotion.']}
    encoded = json.dumps(report, ensure_ascii=False, indent=2, allow_nan=False).encode()
    out=Path(args.output);out.mkdir(parents=True,exist_ok=True)
    (out/'asset-profile-research.json').write_bytes(encoded)
    return hashlib.sha256(encoded).hexdigest()
