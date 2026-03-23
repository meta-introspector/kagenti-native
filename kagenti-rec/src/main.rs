//! kagenti-rec: terminal recording agent — wraps asciinema, publishes casts

use axum::{extract::Json, routing::{get, post}, Router};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::{Arc, Mutex};

fn asciinema_bin() -> String {
    std::env::var("ASCIINEMA_PATH").unwrap_or_else(|_| "asciinema".into())
}

fn spool_dir() -> String {
    std::env::var("REC_SPOOL").unwrap_or_else(|_| "/mnt/data1/spool/rec".into())
}

#[derive(Serialize, Clone)]
struct Recording {
    id: String,
    path: String,
    pid: Option<u32>,
    status: String, // "recording", "done", "error"
}

type Recordings = Arc<Mutex<Vec<Recording>>>;

#[derive(Deserialize)]
struct RecReq {
    name: Option<String>,
    command: Option<String>,
}

async fn health() -> &'static str { "ok" }

async fn list(recs: axum::extract::State<Recordings>) -> Json<Vec<Recording>> {
    Json(recs.lock().unwrap().clone())
}

async fn start_rec(
    recs: axum::extract::State<Recordings>,
    Json(req): Json<RecReq>,
) -> Json<serde_json::Value> {
    let spool = spool_dir();
    let _ = std::fs::create_dir_all(&spool);
    let name = req.name.unwrap_or_else(|| {
        chrono_name()
    });
    let path = format!("{}/{}.cast", spool, name);

    let mut args = vec!["rec".to_string(), path.clone(), "--overwrite".to_string()];
    if let Some(cmd) = req.command {
        args.push("-c".into());
        args.push(cmd);
    }

    match Command::new(asciinema_bin())
        .args(&args)
        .spawn()
    {
        Ok(child) => {
            let pid = child.id();
            let rec = Recording {
                id: name.clone(),
                path: path.clone(),
                pid: Some(pid),
                status: "recording".into(),
            };
            recs.lock().unwrap().push(rec);
            Json(serde_json::json!({"success": true, "id": name, "path": path, "pid": pid}))
        }
        Err(e) => Json(serde_json::json!({"success": false, "error": e.to_string()})),
    }
}

async fn stop_rec(
    recs: axum::extract::State<Recordings>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    let mut recs = recs.lock().unwrap();
    if let Some(rec) = recs.iter_mut().find(|r| r.id == id) {
        if let Some(pid) = rec.pid {
            libc_signal(pid);
            rec.status = "done".into();
            rec.pid = None;
            return Json(serde_json::json!({"success": true, "path": rec.path}));
        }
    }
    Json(serde_json::json!({"success": false, "error": "not found"}))
}

fn libc_signal(pid: u32) {
    // SIGTERM = 15
    let _ = Command::new("kill").arg("-15").arg(pid.to_string()).output();
}

async fn list_casts() -> Json<serde_json::Value> {
    let spool = spool_dir();
    let entries: Vec<String> = std::fs::read_dir(&spool)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "cast").unwrap_or(false))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();
    Json(serde_json::json!({"casts": entries}))
}

async fn cat_cast(
    axum::extract::Path(name): axum::extract::Path<String>,
) -> axum::response::Response {
    let path = format!("{}/{}", spool_dir(), name);
    match std::fs::read_to_string(&path) {
        Ok(data) => axum::response::Response::builder()
            .header("content-type", "application/x-asciicast")
            .body(axum::body::Body::from(data))
            .unwrap(),
        Err(e) => axum::response::Response::builder()
            .status(404)
            .body(axum::body::Body::from(e.to_string()))
            .unwrap(),
    }
}

fn chrono_name() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("rec-{now}")
}

#[tokio::main]
async fn main() {
    let bind = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:8109".into());
    let recs: Recordings = Arc::new(Mutex::new(Vec::new()));

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/recordings", get(list))
        .route("/api/rec", post(start_rec))
        .route("/api/rec/:id/stop", post(stop_rec))
        .route("/api/casts", get(list_casts))
        .route("/api/casts/:name", get(cat_cast))
        .with_state(recs);

    let listener = tokio::net::TcpListener::bind(&bind).await.unwrap();
    println!("kagenti-rec listening on {bind}");
    axum::serve(listener, app).await.unwrap();
}
