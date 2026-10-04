use crate::core::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ResearchState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub interval_secs: u64,
    pub model: String,
    pub next_run: u64,
    pub status: String,
    pub job_id: String,
    pub requested_at: u64,
    pub finished_at: u64,
    pub output: String,
    pub result: Value,
}
impl Default for ResearchState {
    fn default() -> Self {
        Self {
            symbol: None,
            interval_secs: 0,
            model: "score".into(),
            next_run: 0,
            status: "IDLE".into(),
            job_id: String::new(),
            requested_at: 0,
            finished_at: 0,
            output: String::new(),
            result: Value::Null,
        }
    }
}
impl Engine {
    pub fn research_command(&mut self, command: &str, p: Value) -> Result<(), String> {
        match command {
            "research_schedule" => {
                let interval = p["interval_secs"].as_u64().ok_or("缺少 interval_secs")?;
                if interval != 0 && !(3600..=2_592_000).contains(&interval) {
                    return Err("重训周期须为 1 小时到 30 天，0 表示关闭".into());
                }
                let model = p["model"].as_str().ok_or("缺少 model")?;
                if !["score", "logistic", "gbt"].contains(&model) {
                    return Err("未知模型".into());
                }
                if ["QUEUED", "RUNNING"].contains(&self.research.status.as_str()) {
                    return Err("研究任务运行中，请完成后修改计划".into());
                }
                self.research.symbol = p["symbol"]
                    .as_str()
                    .map(crate::universe::normalize)
                    .transpose()?;
                self.research.model = model.into();
                self.research.interval_secs = interval;
                self.research.next_run = self.now + interval * 1000;
            }
            "research_run" => {
                if ["QUEUED", "RUNNING"].contains(&self.research.status.as_str()) {
                    return Err("已有研究任务".into());
                }
                let model = p["model"]
                    .as_str()
                    .unwrap_or(&self.research.model)
                    .to_owned();
                if !["score", "logistic", "gbt"].contains(&model.as_str()) {
                    return Err("未知模型".into());
                }
                if let Some(symbol) = p["symbol"].as_str() {
                    let symbol = crate::universe::normalize(symbol)?;
                    if !self.symbols.contains_key(&symbol) {
                        return Err("币种尚未采集".into());
                    }
                    self.research.symbol = Some(symbol);
                }
                self.research.model = model;
                self.research.status = "QUEUED".into();
                self.research.requested_at = self.now;
                self.counter += 1;
                self.research.job_id = format!("research-{}-{}", self.now, self.counter);
                self.research.result = Value::Null;
            }
            "research_started" => {
                if p["job_id"] != self.research.job_id || self.research.status != "QUEUED" {
                    return Err("任务状态已变化".into());
                }
                self.research.status = "RUNNING".into();
                self.research.output = p["output"].as_str().unwrap_or("").into();
            }
            "research_finished" => {
                if p["job_id"] != self.research.job_id {
                    return Err("任务状态已变化".into());
                }
                self.research.status = if p["ok"] == true {
                    "COMPLETED"
                } else {
                    "FAILED"
                }
                .into();
                self.research.finished_at = self.now;
                self.research.result = p["result"].clone();
                self.research.next_run = self.now + self.research.interval_secs * 1000;
            }
            _ => return Err("未知研究操作".into()),
        }
        Ok(())
    }
    pub fn research_tick(&mut self) {
        if self.research.interval_secs > 0
            && self.now >= self.research.next_run
            && !["QUEUED", "RUNNING"].contains(&self.research.status.as_str())
        {
            let _ = self.command(
                "research_run",
                json!({"model":self.research.model,"scheduled":true}),
            );
        }
    }
}
