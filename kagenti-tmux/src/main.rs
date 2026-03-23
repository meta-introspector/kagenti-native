//! kagenti-tmux: managed tmux sessions with asciinema recording, mcpdesk hooks, zkperf witness

use axum::{extract::{Json, Path, State}, routing::{get, post, delete}, Router};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::process::Command;
use std::sync::{Arc, Mutex};

// --- types ---

#[derive(Clone, Serialize)]
struct Session {
    name: String,
    recording: bool,
    cast_path: Option<String>,
    witness: Option<Witness>,
    systemd_unit: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Witness {
    session_hash: String,
    started_at: u64,
    command: String,
    env_hash: String,
}

#[derive(Deserialize)]
struct CreateReq {
    name: String,
    command: Option<String>,
    record: Option<bool>,
    systemd: Option<bool>,
    env: Option<Vec<(String, String)>>,
}

type Sessions = Arc<Mutex<Vec<Session>>>;

fn tmux_bin() -> String { std::env::var("TMUX_PATH").unwrap_or_else(|_| "tmux".into()) }
fn asciinema_bin() -> String { std::env::var("ASCIINEMA_PATH").unwrap_or_else(|_| "asciinema".into()) }
fn spool_dir() -> String { std::env::var("TMUX_SPOOL").unwrap_or_else(|_| "/mnt/data1/spool/tmux".into()) }

fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    Command::new(cmd).args(args).output()
        .map_err(|e| e.to_string())
        .and_then(|o| if o.status.success() {
            Ok(String::from_utf8_lossy(&o.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&o.stderr).to_string())
        })
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

fn make_witness(name: &str, cmd: &str, env: &[(String, String)]) -> Witness {
    let env_str: String = env.iter().map(|(k,v)| format!("{k}={v}")).collect::<Vec<_>>().join("\n");
    Witness {
        session_hash: hex::encode(Sha256::digest(format!("{name}:{cmd}:{}", now_secs()))),
        started_at: now_secs(),
        command: cmd.into(),
        env_hash: hex::encode(Sha256::digest(&env_str)),
    }
}

// --- handlers ---

async fn health() -> &'static str { "ok" }

async fn list_sessions(State(st): State<Sessions>) -> Json<serde_json::Value> {
    // merge tmux list-sessions with our tracked state
    let tmux_out = run(&tmux_bin(), &["list-sessions", "-F", "#{session_name}"]).unwrap_or_default();
    let tmux_names: Vec<&str> = tmux_out.lines().collect();
    let tracked = st.lock().unwrap();
    Json(serde_json::json!({
        "tmux_sessions": tmux_names,
        "managed": &*tracked,
    }))
}

async fn create_session(
    State(st): State<Sessions>,
    Json(req): Json<CreateReq>,
) -> Json<serde_json::Value> {
    let spool = spool_dir();
    let _ = std::fs::create_dir_all(&spool);
    let cmd = req.command.as_deref().unwrap_or("bash");
    let env = req.env.unwrap_or_default();
    let record = req.record.unwrap_or(true);

    // build witness
    let witness = make_witness(&req.name, cmd, &env);
    // write witness to spool
    let witness_path = format!("{}/{}.witness.json", spool, req.name);
    let _ = std::fs::write(&witness_path, serde_json::to_string_pretty(&witness).unwrap());

    // create tmux session
    let shell_cmd = if record {
        let cast = format!("{}/{}.cast", spool, req.name);
        format!("{} rec {} --overwrite -c '{}'", asciinema_bin(), cast, cmd)
    } else {
        cmd.to_string()
    };

    let res = run(&tmux_bin(), &["new-session", "-d", "-s", &req.name, &shell_cmd]);

    let cast_path = if record { Some(format!("{}/{}.cast", spool, req.name)) } else { None };

    // generate systemd unit if requested
    let unit_name = if req.systemd.unwrap_or(false) {
        let unit = generate_unit(&req.name, &shell_cmd, &env);
        Some(unit)
    } else { None };

    let session = Session {
        name: req.name.clone(),
        recording: record,
        cast_path: cast_path.clone(),
        witness: Some(witness),
        systemd_unit: unit_name.clone(),
    };
    st.lock().unwrap().push(session);

    match res {
        Ok(_) => Json(serde_json::json!({
            "success": true, "session": req.name,
            "recording": record, "cast": cast_path,
            "witness": witness_path, "systemd_unit": unit_name,
        })),
        Err(e) => Json(serde_json::json!({"success": false, "error": e})),
    }
}

async fn kill_session(
    State(st): State<Sessions>,
    Path(name): Path<String>,
) -> Json<serde_json::Value> {
    let res = run(&tmux_bin(), &["kill-session", "-t", &name]);
    st.lock().unwrap().retain(|s| s.name != name);
    match res {
        Ok(_) => Json(serde_json::json!({"success": true})),
        Err(e) => Json(serde_json::json!({"success": false, "error": e})),
    }
}

async fn capture_pane(Path(name): Path<String>) -> Json<serde_json::Value> {
    match run(&tmux_bin(), &["capture-pane", "-p", "-t", &name]) {
        Ok(out) => {
            let hash = hex::encode(Sha256::digest(&out));
            Json(serde_json::json!({"session": name, "content": out.trim(), "hash": hash}))
        }
        Err(e) => Json(serde_json::json!({"error": e})),
    }
}

async fn send_keys(
    Path(name): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let keys = req["keys"].as_str().unwrap_or("");
    match run(&tmux_bin(), &["send-keys", "-t", &name, keys, "Enter"]) {
        Ok(_) => Json(serde_json::json!({"success": true})),
        Err(e) => Json(serde_json::json!({"success": false, "error": e})),
    }
}

async fn get_witness(Path(name): Path<String>) -> Json<serde_json::Value> {
    let path = format!("{}/{}.witness.json", spool_dir(), name);
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or(serde_json::json!({"error": "parse"})),
        Err(e) => serde_json::json!({"error": e.to_string()}),
    }.into()
}

// --- systemd unit generation ---

fn generate_unit(name: &str, cmd: &str, env: &[(String, String)]) -> String {
    let unit_dir = dirs_next().unwrap_or_else(|| "/tmp".into());
    let unit_path = format!("{}/kagenti-tmux-{}.service", unit_dir, name);
    let env_lines: String = env.iter()
        .map(|(k,v)| format!("Environment={}={}", k, v))
        .collect::<Vec<_>>().join("\n");

    let unit = format!(r#"[Unit]
Description=kagenti-tmux session: {name}
After=network.target

[Service]
Type=forking
ExecStart={tmux} new-session -d -s {name} '{cmd}'
ExecStop={tmux} kill-session -t {name}
{env_lines}
Restart=on-failure

[Install]
WantedBy=default.target
"#, tmux = tmux_bin());

    let _ = std::fs::write(&unit_path, &unit);
    // reload
    let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).output();
    unit_path
}

fn dirs_next() -> Option<String> {
    std::env::var("HOME").ok().map(|h| format!("{}/.config/systemd/user", h))
}

#[tokio::main]
async fn main() {
    let bind = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:8110".into());
    let sessions: Sessions = Arc::new(Mutex::new(Vec::new()));

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/sessions", get(list_sessions))
        .route("/api/sessions", post(create_session))
        .route("/api/sessions/:name", delete(kill_session))
        .route("/api/sessions/:name/capture", get(capture_pane))
        .route("/api/sessions/:name/send", post(send_keys))
        .route("/api/sessions/:name/witness", get(get_witness))
        .with_state(sessions);

    let listener = tokio::net::TcpListener::bind(&bind).await.unwrap();
    println!("kagenti-tmux listening on {bind}");
    axum::serve(listener, app).await.unwrap();
}
