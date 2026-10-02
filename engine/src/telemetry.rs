use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
#[derive(Clone, Debug, Default)]
pub struct Telemetry {
    windows: BTreeMap<String, VecDeque<u64>>,
}
impl Telemetry {
    pub fn record(&mut self, name: &str, micros: u64) {
        let w = self.windows.entry(name.into()).or_default();
        w.push_back(micros);
        if w.len() > 1000 {
            w.pop_front();
        }
    }
    pub fn view(&self) -> Value {
        let map:BTreeMap<_,_>=self.windows.iter().map(|(name,w)|{let mut v:Vec<_>=w.iter().copied().collect();v.sort_unstable();
            (name,json!({"samples":v.len(),"p50_us":v[v.len()/2],"p95_us":v[(v.len()*95/100).min(v.len()-1)],"max_us":v.last()}))}).collect();
        json!(map)
    }
}
