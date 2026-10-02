import importlib.util
import json
import tempfile
import unittest
import contextlib
import io
import sqlite3
import copy
from unittest.mock import patch
from argparse import Namespace
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
def module(name,path):
    spec=importlib.util.spec_from_file_location(name,path)
    obj=importlib.util.module_from_spec(spec);spec.loader.exec_module(obj)
    return obj
train=module('research_train',ROOT/'research/train.py')
parquet=module('research_parquet',ROOT/'research/parquet.py')

def synthetic_rows(count=800):
    rng = train.np.random.default_rng(19)
    rows = []
    for i in range(count):
        f = dict(book_imbalance=float(rng.uniform(-.8,.8)), flow_imbalance=0., aggressive_imbalance=0.,
                 microprice_bps=0., momentum=0., volatility=.0002, cross_venue_bps=0., spread_bps=1., depth=1e6)
        p = dict(prediction_id=f'row-{i}', ts=i*1000, symbol='BTCUSDT' if i%2 else 'ETHUSDT',
                 direction='LONG' if f['book_imbalance'] >= 0 else 'SHORT', target=.003, stop=.001,
                 features=f, qualification={'costs': {'total': .0012}})
        win = i%5 < 3
        rows.append(dict(prediction=p, first_hit_ts=i*1000+12000, outcome_type='TARGET_HIT' if win else 'STOP_HIT',
                         observed_net_edge=.0018 if win else -.0022))
    return rows

class ResearchTests(unittest.TestCase):
    def test_walk_forward_refits_with_purged_boundaries_and_disjoint_tests(self):
        folds = train.walk_forward(synthetic_rows())
        seen = set(); sizes = []
        for training, cal, test in folds:
            sizes.append(len(training))
            self.assertLess(max(r['first_hit_ts'] for r in training), min(r['prediction']['ts'] for r in cal))
            self.assertLess(max(r['first_hit_ts'] for r in cal), min(r['prediction']['ts'] for r in test))
            ids = {r['prediction']['prediction_id'] for r in test}
            self.assertFalse(ids & seen); seen |= ids
        self.assertEqual(len(folds), 3)
        self.assertTrue(all(a < b for a,b in zip(sizes,sizes[1:])))

    def test_future_data_cannot_change_weights_distribution_or_calibration(self):
        training, cal, test = train.walk_forward(synthetic_rows())[0]
        weights, profile, fitted = train.fit_fold(training, cal, test)
        future = copy.deepcopy(test)
        for row in future:
            row['prediction']['features']['momentum'] = 1.
            row['prediction']['features']['volatility'] = .2
            row['outcome_type'] = 'STRONG_OPPOSITE_MOVE'
        changed_weights, changed_profile, changed_fit = train.fit_fold(training, cal, future)
        train.np.testing.assert_array_equal(weights, changed_weights)
        self.assertEqual(profile, changed_profile)
        for name in fitted:
            self.assertEqual(fitted[name]['calibrations'], changed_fit[name]['calibrations'])
            self.assertEqual(fitted[name]['temperature'], changed_fit[name]['temperature'])
            self.assertFalse(changed_fit[name]['eligible'].any())
        self.assertTrue(all(v > 0 for d in profile.values() for v in d['scale']))

    def test_sparse_regime_has_no_calibration_and_ood_cannot_be_selected(self):
        rows = synthetic_rows(100)
        profile = train.distribution_profile(rows)
        logits = train.np.array([train.score_logits(r['prediction']['features']) for r in rows])
        labels = train.np.array([train.target(r) for r in rows])
        self.assertEqual(train.fit_regimes(rows[:10], logits[:10], labels[:10], profile), {})
        calibrations = train.fit_regimes(rows, logits, labels, profile)
        self.assertEqual(len(calibrations), 2)
        changed = copy.deepcopy(rows[:2])
        changed[0]['prediction']['features']['momentum'] = 1.
        changed[1]['prediction']['features']['volatility'] = 1.
        probs, eligible = train.regime_probabilities(changed, logits[:2], profile, calibrations)
        self.assertFalse(eligible.any())
        train.np.testing.assert_allclose(probs[1], train.softmax(logits[1:2])[0])

    def test_temporal_split_purges_outcomes_crossing_boundaries(self):
        rows=[{'prediction':{'ts':i*1000},'first_hit_ts':i*1000+12_000} for i in range(300)]
        a,b,c=train.split_rows(rows)
        self.assertLess(max(r['first_hit_ts'] for r in a),min(r['prediction']['ts'] for r in b))
        self.assertLess(max(r['first_hit_ts'] for r in b),min(r['prediction']['ts'] for r in c))
        self.assertLess(len(a)+len(b)+len(c),len(rows))

    def test_logistic_fit_learns_without_changing_input_data(self):
        np=train.np;rng=np.random.default_rng(7);x=np.column_stack([np.ones(600),rng.normal(size=(600,9))]);before=x.copy()
        y=np.argmax(np.column_stack([x[:,1]*2,-x[:,1]*2,x[:,2]*2]),axis=1)
        w=train.fit(x[:400],y[:400]);p=train.softmax(x[400:]@w.T)
        self.assertGreater((p.argmax(axis=1)==y[400:]).mean(),.8)
        np.testing.assert_array_equal(x,before)
        np.testing.assert_allclose(p.sum(axis=1),1.)

    def test_temperature_calibration_reduces_overconfidence(self):
        np=train.np;logits=np.tile([4.,0.,0.],(100,1));labels=np.array([0]*60+[1]*20+[2]*20)
        temp=train.calibrate(logits,labels)
        self.assertGreater(temp,1.)
        before=-np.log(train.softmax(logits)[np.arange(100),labels]).mean()
        after=-np.log(train.softmax(logits,temp)[np.arange(100),labels]).mean()
        self.assertLess(after,before)

    def test_parquet_roundtrip_preserves_market_event_payload(self):
        import pyarrow.parquet as pq
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);source=root/'fixture.jsonl'
            event={'exchange':'okx','symbol':'BTCUSDT','event_type':'delta','exchange_ts':123,'receive_ts':125,'sequence_id':2,
                   'previous':1,'first':2,'bids':[[100.1,0.01]],'asks':[]}
            source.write_text(json.dumps({'type':'event','event':event})+'\n')
            parquet.export(source,root/'out');files=list((root/'out').rglob('*.parquet'));self.assertEqual(len(files),1)
            rows=pq.read_table(files[0]).to_pylist();payload=json.loads(rows[0].pop('payload'))
            self.assertEqual({**rows[0],**payload},event)

    def test_partial_recording_is_rejected_in_batch_export(self):
        with tempfile.TemporaryDirectory() as d:
            path=Path(d)/'broken.jsonl';path.write_text('{"type":"event"')
            with self.assertRaises(ValueError):parquet.export(path,Path(d)/'out')

    def test_full_offline_report_and_source_isolation(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);db=root/'demo.sqlite';rng=train.np.random.default_rng(11)
            with sqlite3.connect(db) as conn:
                conn.execute('CREATE TABLE records(id INTEGER PRIMARY KEY,kind TEXT,ts INTEGER,payload TEXT)')
                for i in range(400):
                    f=dict(book_imbalance=float(rng.uniform(-.8,.8)),flow_imbalance=0.,aggressive_imbalance=0.,microprice_bps=0.,momentum=0.,volatility=.0002,cross_venue_bps=0.,spread_bps=1.,depth=1e6)
                    long=f['book_imbalance']>=0
                    outcome='TARGET_HIT' if i%5<3 else 'STOP_HIT'
                    p=dict(prediction_id=f'synthetic-{i}',ts=i*60000,symbol='BTCUSDT' if i%2 else 'ETHUSDT',direction='LONG' if long else 'SHORT',
                           config_version=1,horizon=30,target=.003,stop=.001,features=f,qualification={'costs':{'total':.0012,'spread':.0001}})
                    o=dict(prediction=p,first_hit_ts=p['ts']+5000,outcome_type=outcome,observed_net_edge=.0018 if outcome=='TARGET_HIT' else -.0022,learning_eligible=True)
                    conn.execute('INSERT INTO records(kind,ts,payload) VALUES(?,?,?)',('outcome',p['ts'],json.dumps({'source':'demo','payload':o})))
            args=Namespace(db=str(db),source='demo',model='logistic',output=str(root/'model'))
            with contextlib.redirect_stdout(io.StringIO()), patch.object(train, 'fit', wraps=train.fit) as fitted:
                train.run(args)
                self.assertEqual(fitted.call_count, 3)
            artifact=json.loads((root/'model/challenger.json').read_text());report=json.loads((root/'model/report.json').read_text())
            self.assertTrue(report['walk_forward']['refit_each_fold'])
            self.assertIn('distributions', artifact)
            self.assertIn('regime_calibrations', artifact)
            self.assertEqual(artifact['source'],'demo');self.assertEqual(len(artifact['weights']),3)
            self.assertEqual(len(report['chronological_test_windows']),3);self.assertIn(report['status'],['REJECTED','PAPER_RESEARCH_GATE_PASS'])
            args.source='live'
            with self.assertRaisesRegex(ValueError,'没有匹配来源'):train.run(args)

if __name__=='__main__':unittest.main()
