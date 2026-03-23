//! kagenti-wg: WireGuard management agent for kagenti

use axum::{extract::Path, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize)]
struct WgInterface {
    name: String,
    public_key: String,
    listen_port: String,
    peers: Vec<WgPeer>,
}

#[derive(Serialize)]
struct WgPeer {
    public_key: String,
    endpoint: String,
    allowed_ips: String,
    latest_handshake: String,
    transfer_rx: String,
    transfer_tx: String,
}

#[derive(Deserialize)]
struct GenKeyReq {
    name: Option<String>,
}

#[derive(Deserialize)]
struct PeerReq {
    interface: String,
    public_key: String,
    endpoint: Option<String>,
    allowed_ips: String,
    keepalive: Option<u16>,
}

fn wg_bin() -> String {
    std::env::var("WG_PATH").unwrap_or_else(|_| "wg".into())
}

fn wg_quick_bin() -> String {
    // wg-quick lives next to wg
    let wg = wg_bin();
    if wg.contains('/') {
        wg.replace("/wg", "/wg-quick")
    } else {
        "wg-quick".into()
    }
}

fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    Command::new(cmd).args(args).output()
        .map_err(|e| e.to_string())
        .and_then(|o| if o.status.success() {
            Ok(String::from_utf8_lossy(&o.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&o.stderr).to_string())
        })
}

async fn health() -> &'static str { "ok" }

async fn show_all() -> Json<serde_json::Value> {
    match run(&wg_bin(), &["show", "all", "dump"]) {
        Ok(out) => {
            let mut ifaces: Vec<WgInterface> = Vec::new();
            for line in out.lines() {
                let f: Vec<&str> = line.split('\t').collect();
                if f.len() == 5 {
                    // interface line
                    ifaces.push(WgInterface {
                        name: f[0].into(), public_key: f[1].into(),
                        listen_port: f[3].into(), peers: Vec::new(),
                    });
                } else if f.len() >= 8 {
                    // peer line
                    if let Some(iface) = ifaces.last_mut() {
                        iface.peers.push(WgPeer {
                            public_key: f[1].into(), endpoint: f[3].into(),
                            allowed_ips: f[4].into(), latest_handshake: f[5].into(),
                            transfer_rx: f[6].into(), transfer_tx: f[7].into(),
                        });
                    }
                }
            }
            Json(serde_json::json!({"interfaces": ifaces}))
        }
        Err(e) => Json(serde_json::json!({"interfaces": [], "note": e})),
    }
}

async fn show_iface(Path(name): Path<String>) -> Json<serde_json::Value> {
    match run(&wg_bin(), &["show", &name, "dump"]) {
        Ok(out) => Json(serde_json::json!({"interface": name, "dump": out.trim()})),
        Err(e) => Json(serde_json::json!({"error": e})),
    }
}

async fn genkey(Json(req): Json<GenKeyReq>) -> Json<serde_json::Value> {
    match run(&wg_bin(), &["genkey"]) {
        Ok(priv_key) => {
            let priv_key = priv_key.trim();
            let wg = wg_bin();
            let pub_key = Command::new(&wg).arg("pubkey")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn().and_then(|mut c| {
                    use std::io::Write;
                    c.stdin.take().unwrap().write_all(priv_key.as_bytes()).ok();
                    c.wait_with_output()
                }).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            Json(serde_json::json!({
                "name": req.name.unwrap_or_default(),
                "private_key": priv_key, "public_key": pub_key,
            }))
        }
        Err(e) => Json(serde_json::json!({"error": e})),
    }
}

async fn up(Path(name): Path<String>) -> Json<serde_json::Value> {
    match run(&wg_quick_bin(), &["up", &name]) {
        Ok(out) => Json(serde_json::json!({"success": true, "output": out.trim()})),
        Err(e) => Json(serde_json::json!({"success": false, "error": e})),
    }
}

async fn down(Path(name): Path<String>) -> Json<serde_json::Value> {
    match run(&wg_quick_bin(), &["down", &name]) {
        Ok(out) => Json(serde_json::json!({"success": true, "output": out.trim()})),
        Err(e) => Json(serde_json::json!({"success": false, "error": e})),
    }
}

async fn add_peer(Json(req): Json<PeerReq>) -> Json<serde_json::Value> {
    let wg = wg_bin();
    let mut args = vec!["set", &req.interface, "peer", &req.public_key, "allowed-ips", &req.allowed_ips];
    let ep;
    if let Some(ref e) = req.endpoint {
        args.extend_from_slice(&["endpoint", e.as_str()]);
        ep = e.clone();
    } else { ep = String::new(); }
    let ka_str;
    if let Some(ka) = req.keepalive {
        ka_str = ka.to_string();
        args.extend_from_slice(&["persistent-keepalive", &ka_str]);
    }
    match run(&wg, &args) {
        Ok(_) => Json(serde_json::json!({"success": true, "peer": req.public_key, "endpoint": ep})),
        Err(e) => Json(serde_json::json!({"success": false, "error": e})),
    }
}

async fn remove_peer(Path((iface, pubkey)): Path<(String, String)>) -> Json<serde_json::Value> {
    match run(&wg_bin(), &["set", &iface, "peer", &pubkey, "remove"]) {
        Ok(_) => Json(serde_json::json!({"success": true})),
        Err(e) => Json(serde_json::json!({"success": false, "error": e})),
    }
}

#[tokio::main]
async fn main() {
    let bind = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:8108".into());

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/show", get(show_all))
        .route("/api/show/:name", get(show_iface))
        .route("/api/genkey", post(genkey))
        .route("/api/up/:name", post(up))
        .route("/api/down/:name", post(down))
        .route("/api/peer", post(add_peer))
        .route("/api/peer/:iface/:pubkey", axum::routing::delete(remove_peer));

    let listener = tokio::net::TcpListener::bind(&bind).await.unwrap();
    println!("kagenti-wg listening on {bind}");
    axum::serve(listener, app).await.unwrap();
}
