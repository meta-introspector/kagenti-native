//! kagenti-ctl: CLI for kagenti-daemon agent management.
//! Replaces Makefile — all procedures as checked-in Rust.

use serde_json::Value;
use std::{env, fs, path::PathBuf, process};

const DEFAULT_DAEMON: &str = "http://localhost:8480";
const DEFAULT_NS: &str = "default";

fn daemon() -> String {
    env::var("KAGENTI_DAEMON").unwrap_or_else(|_| DEFAULT_DAEMON.into())
}

fn agents_dir() -> PathBuf {
    let mut p = env::current_exe().unwrap_or_default();
    // Walk up from target/release/kagenti-ctl to workspace root
    for _ in 0..3 { p.pop(); }
    let d = p.join("agents");
    if d.is_dir() { return d; }
    // Fallback: cwd/agents
    PathBuf::from("agents")
}

fn load_manifest(name: &str) -> Value {
    let path = agents_dir().join(format!("{name}.json"));
    let data = fs::read_to_string(&path)
        .unwrap_or_else(|e| { eprintln!("error: {}: {e}", path.display()); process::exit(1); });
    serde_json::from_str(&data)
        .unwrap_or_else(|e| { eprintln!("error: parse {}: {e}", path.display()); process::exit(1); })
}

fn all_agent_names() -> Vec<String> {
    let dir = agents_dir();
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| { eprintln!("error: {}: {e}", dir.display()); process::exit(1); })
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if p.extension().map_or(false, |x| x == "json") {
                p.file_stem().map(|s| s.to_string_lossy().into_owned())
            } else { None }
        })
        .collect();
    names.sort();
    names
}

fn post(url: &str, body: &Value) -> Result<Value, String> {
    ureq::post(url)
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(|e| e.to_string())
        .and_then(|r| r.into_string().map_err(|e| e.to_string()))
        .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
}

fn get(url: &str) -> Result<Value, String> {
    ureq::get(url)
        .call()
        .map_err(|e| e.to_string())
        .and_then(|r| r.into_string().map_err(|e| e.to_string()))
        .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
}

fn print_json(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap());
}

fn cmd_register(name: &str) {
    let manifest = load_manifest(name);
    let summary = serde_json::json!({
        "name": manifest["name"],
        "namespace": manifest.get("namespace").and_then(|v| v.as_str()).unwrap_or(DEFAULT_NS),
        "description": format!("kagenti-managed agent: {name}"),
        "status": "stopped",
        "labels": {"protocol": ["http"], "type": "deployment"},
    });
    let url = format!("{}/apis/kagenti/v1/namespaces/{DEFAULT_NS}/agents", daemon());
    match post(&url, &summary) {
        Ok(v) => print_json(&v),
        Err(e) => eprintln!("register {name}: {e}"),
    }
}

fn cmd_deploy(name: &str) {
    let manifest = load_manifest(name);
    let url = format!("{}/apis/kagenti/v1/deploy", daemon());
    match post(&url, &manifest) {
        Ok(v) => print_json(&v),
        Err(e) => eprintln!("deploy {name}: {e}"),
    }
}

fn cmd_list() {
    let url = format!("{}/apis/kagenti/v1/namespaces/{DEFAULT_NS}/agents", daemon());
    match get(&url) {
        Ok(v) => print_json(&v),
        Err(e) => eprintln!("list: {e}"),
    }
}

fn cmd_lifecycle(action: &str, name: &str) {
    let url = match action {
        "start" | "stop" => format!("{}/apis/kagenti/v1/agents/{name}/{action}", daemon()),
        "status" => format!("{}/apis/kagenti/v1/agents/{name}/status", daemon()),
        _ => unreachable!(),
    };
    let result = if action == "status" { get(&url) } else { post(&url, &serde_json::json!({})) };
    match result {
        Ok(v) => print_json(&v),
        Err(e) => eprintln!("{action} {name}: {e}"),
    }
}

fn usage() {
    eprintln!("kagenti-ctl — agent lifecycle manager");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  kagenti-ctl register <name>    Register agent from agents/<name>.json");
    eprintln!("  kagenti-ctl register-all       Register all agents in agents/");
    eprintln!("  kagenti-ctl deploy <name>      Deploy agent (systemd install + start)");
    eprintln!("  kagenti-ctl deploy-all         Deploy all agents");
    eprintln!("  kagenti-ctl list               List registered agents");
    eprintln!("  kagenti-ctl start <name>       Start agent");
    eprintln!("  kagenti-ctl stop <name>        Stop agent");
    eprintln!("  kagenti-ctl status <name>      Agent status");
    eprintln!();
    eprintln!("Env: KAGENTI_DAEMON (default: {DEFAULT_DAEMON})");
    process::exit(1);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 { usage(); }

    match args[1].as_str() {
        "register" if args.len() == 3 => cmd_register(&args[2]),
        "register-all" => for name in all_agent_names() { cmd_register(&name); },
        "deploy" if args.len() == 3 => cmd_deploy(&args[2]),
        "deploy-all" => for name in all_agent_names() { cmd_deploy(&name); },
        "list" => cmd_list(),
        "start" | "stop" | "status" if args.len() == 3 => cmd_lifecycle(&args[1], &args[2]),
        _ => usage(),
    }
}
