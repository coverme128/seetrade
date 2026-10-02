use crate::core::Engine;
use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Journal {
    Start {
        engine: Box<Engine>,
    },
    Event {
        event: crate::types::MarketEvent,
    },
    Command {
        command: String,
        payload: Value,
    },
    RejectedCommand {
        command: String,
        payload: Value,
        error: String,
    },
}
pub struct Batch {
    pub journal: Journal,
    pub records: Vec<Value>,
    pub checkpoint: Option<String>,
    pub committed: Option<tokio::sync::oneshot::Sender<Result<(), String>>>,
}
pub struct Store {
    pub tx: mpsc::Sender<Batch>,
    pub healthy: Arc<AtomicBool>,
    pub path: PathBuf,
    pub journal_path: PathBuf,
}
impl Store {
    pub fn open(dir: &Path, source: &str) -> Result<(Self, Option<Engine>)> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{source}.sqlite"));
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS records(id INTEGER PRIMARY KEY, kind TEXT NOT NULL, ts INTEGER NOT NULL, payload TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS records_kind ON records(kind, id);
            CREATE TABLE IF NOT EXISTS checkpoint(id INTEGER PRIMARY KEY CHECK(id=1), state TEXT NOT NULL);")?;
        let restored = conn
            .query_row("SELECT state FROM checkpoint WHERE id=1", [], |r| {
                r.get::<_, String>(0)
            })
            .ok()
            .map(|s| serde_json::from_str::<Engine>(&s))
            .transpose()?;
        let journal_path = dir.join(format!("{source}-{}.jsonl", crate::feeds::now_ms()));
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&journal_path)?;
        let (tx, mut rx) = mpsc::channel::<Batch>(8192);
        let healthy = Arc::new(AtomicBool::new(true));
        let health = healthy.clone();
        std::thread::spawn(move || {
            let mut writer = BufWriter::new(file);
            let mut conn = conn;
            while let Some(mut batch) = rx.blocking_recv() {
                let committed = batch.committed.take();
                match write_batch(&mut conn, &mut writer, batch) {
                    Ok(()) => {
                        if let Some(ack) = committed {
                            let _ = ack.send(Ok(()));
                        }
                    }
                    Err(e) => {
                        tracing::error!("storage failure: {e}");
                        health.store(false, Ordering::SeqCst);
                        if let Some(ack) = committed {
                            let _ = ack.send(Err(e.to_string()));
                        }
                        break;
                    }
                }
            }
            let _ = writer.flush();
            let _ = writer.get_ref().sync_all();
        });
        Ok((
            Self {
                tx,
                healthy,
                path,
                journal_path,
            },
            restored,
        ))
    }
    pub fn history(path: &Path, kind: &str, limit: usize) -> Result<Vec<Value>> {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut stmt =
            conn.prepare("SELECT payload FROM records WHERE kind=?1 ORDER BY id DESC LIMIT ?2")?;
        let rows = stmt.query_map(params![kind, limit.min(500)], |row| row.get::<_, String>(0))?;
        let mut results: Vec<Value> = rows
            .map(|r| Ok(serde_json::from_str(&r?)?))
            .collect::<Result<_>>()?;
        if kind == "outcome" {
            let mut stmt =
                conn.prepare("SELECT payload FROM records WHERE kind='annotate' ORDER BY id")?;
            let notes = stmt.query_map([], |row| row.get::<_, String>(0))?;
            let mut annotations = std::collections::BTreeMap::new();
            for note in notes {
                let note: Value = serde_json::from_str(&note?)?;
                if let Some(id) = note["payload"]["prediction_id"].as_str() {
                    annotations.insert(id.to_owned(), note["payload"]["annotation"].clone());
                }
            }
            let mut horizon_stmt = conn
                .prepare("SELECT payload FROM records WHERE kind='outcome_horizon' ORDER BY id")?;
            let rows = horizon_stmt.query_map([], |row| row.get::<_, String>(0))?;
            let mut horizons = std::collections::BTreeMap::new();
            for row in rows {
                let record: Value = serde_json::from_str(&row?)?;
                if let Some(id) = record["payload"]["prediction_id"].as_str() {
                    horizons.insert(id.to_owned(), record["payload"]["observation"].clone());
                }
            }
            for result in &mut results {
                if let Some(observation) = result["payload"]["prediction"]["prediction_id"]
                    .as_str()
                    .and_then(|id| horizons.get(id))
                {
                    result["payload"]["horizon_observation"] = observation.clone();
                }

                if let Some(note) = result["payload"]["prediction"]["prediction_id"]
                    .as_str()
                    .and_then(|id| annotations.get(id))
                {
                    result["payload"]["annotation"] = note.clone();
                }
            }
        }
        Ok(results)
    }
}
fn write_batch(conn: &mut Connection, writer: &mut BufWriter<File>, batch: Batch) -> Result<()> {
    serde_json::to_writer(&mut *writer, &batch.journal)?;
    writer.write_all(b"\n")?;
    let transaction = conn.transaction()?;
    for record in batch.records {
        transaction.execute(
            "INSERT INTO records(kind,ts,payload) VALUES(?1,?2,?3)",
            params![
                record["kind"].as_str(),
                record["ts"].as_u64(),
                record.to_string()
            ],
        )?;
    }
    if let Some(state) = batch.checkpoint {
        writer.flush()?;
        writer.get_ref().sync_data()?;
        transaction.execute("INSERT INTO checkpoint(id,state) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET state=excluded.state",[state])?;
    }
    transaction.commit()?;
    Ok(())
}
