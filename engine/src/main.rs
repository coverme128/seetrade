use anyhow::{Context, Result};
use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use micro_alpha_engine::{
    core::Engine,
    feeds,
    model::Model,
    storage::{Batch, Journal, Store},
    types::*,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
};
use tokio::sync::{mpsc, Mutex};

struct App {
    engine: Mutex<Engine>,
    store: Store,
}
type Shared = Arc<App>;
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args: Vec<_> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--replay") {
        return replay(args.get(i + 1).context("--replay needs file")?);
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8787")
        .await
        .context("engine port 8787 unavailable")?;
    let demo = args.iter().any(|a| a == "--demo");
    let source = if demo { "demo" } else { "live" };
    let dir = std::env::var("MICRO_ALPHA_DATA").unwrap_or_else(|_| "data".into());
    let (store, restored) = Store::open(std::path::Path::new(&dir), source)?;
    let mut engine = restored.unwrap_or_else(|| Engine::new(demo));
    if engine.now > 0 {
        engine.restart();
    }
    if let Some(i) = args.iter().position(|a| a == "--model") {
        let bytes = std::fs::read(args.get(i + 1).context("--model needs artifact file")?)?;
        let mut model: Model = serde_json::from_slice(&bytes)?;
        model.validate(demo).map_err(anyhow::Error::msg)?;
        model.artifact_hash = format!("{:x}", Sha256::digest(&bytes));
        engine.log("model_load", json!(&model));
        engine.model = model;
    }
    let initial = serde_json::to_string(&engine)?;
    store
        .tx
        .send(Batch {
            journal: Journal::Start {
                engine: Box::new(engine.clone()),
            },
            records: std::mem::take(&mut engine.records),
            checkpoint: Some(initial),
            committed: None,
        })
        .await?;
    tracing::info!(journal=%store.journal_path.display(),"session recording started");
    let app = Arc::new(App {
        engine: Mutex::new(engine),
        store,
    });
    let (tx, mut rx) = mpsc::channel(8192);
    if demo {
        let demo_tx = tx.clone();
        tokio::spawn(async move {
            let mut step = 0;
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                for e in feeds::demo_events(step, feeds::now_ms()) {
                    if demo_tx.send(e).await.is_err() {
                        return;
                    }
                }
                step += 1;
            }
        });
    } else {
        feeds::spawn_live(tx.clone());
        let clock_tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                let ts = feeds::now_ms();
                if clock_tx
                    .send(MarketEvent {
                        exchange: "clock".into(),
                        symbol: String::new(),
                        exchange_ts: ts,
                        receive_ts: ts,
                        sequence_id: 0,
                        data: EventData::Clock,
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
    }
    let runtime = app.clone();
    let worker = tokio::spawn(async move {
        while let Some(mut e) = rx.recv().await {
            let mut engine = runtime.engine.lock().await;
            // Serialize receive order across independent venue sockets before recording/replay.
            e.receive_ts = e.receive_ts.max(engine.now);
            let checkpoint = matches!(e.data, EventData::Clock);
            engine.storage_healthy = runtime.store.healthy.load(Ordering::SeqCst);
            if !engine.storage_healthy {
                engine.kill_switch = true;
                engine.mode = "OBSERVE".into();
            }
            engine.ingest(e.clone());
            persist(
                &runtime,
                &mut engine,
                Journal::Event { event: e },
                checkpoint,
            );
        }
    });
    let router = Router::new()
        .route("/api/state", get(state))
        .route("/api/health", get(health))
        .route("/api/history", get(history))
        .route("/api/control/{command}", post(control))
        .layer(DefaultBodyLimit::max(32 * 1024))
        .with_state(app.clone());
    tracing::info!("Micro Alpha running at 127.0.0.1:8787 · source={source} · mode=OBSERVE");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    worker.abort();
    let _ = worker.await;
    let (commit_tx, commit_rx) = tokio::sync::oneshot::channel();
    {
        let mut engine = app.engine.lock().await;
        engine
            .command("mode", json!({"mode":"OBSERVE"}))
            .map_err(anyhow::Error::msg)?;
        let checkpoint = serde_json::to_string(&*engine)?;
        let records = std::mem::take(&mut engine.records);
        app.store
            .tx
            .send(Batch {
                journal: Journal::Command {
                    command: "mode".into(),
                    payload: json!({"mode":"OBSERVE"}),
                },
                records,
                checkpoint: Some(checkpoint),
                committed: Some(commit_tx),
            })
            .await?;
    }
    commit_rx.await?.map_err(anyhow::Error::msg)?;
    Ok(())
}
fn persist(app: &App, engine: &mut Engine, journal: Journal, checkpoint: bool) {
    let snapshot = if checkpoint || !engine.records.is_empty() {
        Some(serde_json::to_string(&*engine).expect("finite engine state"))
    } else {
        None
    };
    let records = std::mem::take(&mut engine.records);
    if app
        .store
        .tx
        .try_send(Batch {
            journal,
            records,
            checkpoint: snapshot,
            committed: None,
        })
        .is_err()
    {
        engine.storage_healthy = false;
        engine.kill_switch = true;
        engine.mode = "OBSERVE".into();
        app.store.healthy.store(false, Ordering::SeqCst);
        tracing::error!("recorder queue full or disconnected; new paper orders halted");
    }
}
async fn state(State(app): State<Shared>) -> Json<Value> {
    Json(app.engine.lock().await.view())
}
async fn health(State(app): State<Shared>) -> Json<Value> {
    let e = app.engine.lock().await;
    Json(json!({"ok":e.storage_healthy,"source":e.source,"live_enabled":false}))
}
async fn control(
    State(app): State<Shared>,
    Path(command): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let err = |s: StatusCode, m: String| (s, Json(json!({"error":m})));
    if headers.get("x-micro-alpha").and_then(|v| v.to_str().ok()) != Some("local") {
        return Err(err(StatusCode::FORBIDDEN, "缺少本地控制请求标记".into()));
    }
    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        if ![
            "http://127.0.0.1:3000",
            "http://localhost:3000",
            "http://127.0.0.1:8787",
        ]
        .contains(&origin)
        {
            return Err(err(StatusCode::FORBIDDEN, "仅允许本机界面控制".into()));
        }
    }
    let mut engine = app.engine.lock().await;
    if !app.store.healthy.load(Ordering::SeqCst) {
        return Err(err(
            StatusCode::SERVICE_UNAVAILABLE,
            "存储不可用，控制操作已暂停".into(),
        ));
    }
    // Reserve recording capacity before accepting a mutation.
    let permit = app.store.tx.try_reserve().map_err(|_| {
        err(
            StatusCode::SERVICE_UNAVAILABLE,
            "记录队列繁忙，请稍后重试".into(),
        )
    })?;
    engine
        .command(&command, payload.clone())
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    let checkpoint = serde_json::to_string(&*engine)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let (commit_tx, commit_rx) = tokio::sync::oneshot::channel();
    permit.send(Batch {
        journal: Journal::Command { command, payload },
        records: std::mem::take(&mut engine.records),
        checkpoint: Some(checkpoint),
        committed: Some(commit_tx),
    });
    let view = engine.view();
    drop(engine);
    // Durable confirmation happens outside the trading-core lock.
    commit_rx
        .await
        .map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "存储服务已停止".into()))?
        .map_err(|e| err(StatusCode::SERVICE_UNAVAILABLE, e))?;
    Ok(Json(view))
}
#[derive(Deserialize)]
struct HistoryQuery {
    kind: Option<String>,
    limit: Option<usize>,
}
async fn history(
    State(app): State<Shared>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<Value>, StatusCode> {
    let path = app.store.path.clone();
    let kind = q.kind.unwrap_or_else(|| "outcome".into());
    let limit = q.limit.unwrap_or(100);
    let rows = tokio::task::spawn_blocking(move || Store::history(&path, &kind, limit))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!(rows)))
}
fn replay(path: &str) -> Result<()> {
    let file = BufReader::new(File::open(PathBuf::from(path))?);
    let mut engine = None;
    let mut count = 0;
    for line in file.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Journal>(&line)
            .with_context(|| format!("invalid journal line {}", count + 1))?
        {
            Journal::Start { engine: e } => engine = Some(*e),
            Journal::Event { event } => engine
                .as_mut()
                .context("journal missing start")?
                .ingest(event),
            Journal::Command { command, payload } => engine
                .as_mut()
                .context("journal missing start")?
                .command(&command, payload)
                .map_err(anyhow::Error::msg)?,
        }
        if let Some(e) = &mut engine {
            e.records.clear();
        }
        count += 1;
    }
    let engine = engine.context("empty recording")?;
    let bytes = serde_json::to_vec(&engine)?;
    println!(
        "{}",
        json!({"records":count,"state_sha256":format!("{:x}",Sha256::digest(bytes)),"outcomes":engine.outcomes.len(),"trades":engine.trades.len(),"source":engine.source})
    );
    Ok(())
}
