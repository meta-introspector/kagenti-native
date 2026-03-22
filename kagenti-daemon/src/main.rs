//! kagenti-daemon: API server + reconciler

mod systemd;

use axum::{extract::{Path, Json, State}, routing::{get, delete, post}, Router};
use kagenti_api::{routes, types::AgentRuntime};
use kagenti_rmq::{AgentEvent, EventBus};
use std::sync::{Arc, Mutex};

const RMQ_URI: &str = "amqp://kagenti:kagenti@localhost:5672/%2Fmonster";

#[derive(Clone)]
struct AppState {
    agents: routes::Store,
    runtimes: Arc<Mutex<Vec<AgentRuntime>>>,
    rmq: Option<Arc<EventBus>>,
}

// -- systemd management routes --

async fn deploy_agent(State(st): State<AppState>, Json(rt): Json<AgentRuntime>) -> Json<serde_json::Value> {
    if let Err(e) = systemd::install(&rt).and_then(|_| systemd::start(&rt)) {
        return Json(serde_json::json!({"success": false, "message": e.to_string()}));
    }
    st.runtimes.lock().unwrap().push(rt.clone());
    // Publish event if RMQ connected
    if let Some(bus) = &st.rmq {
        let bus = bus.clone();
        let event = AgentEvent::Created(kagenti_api::types::AgentSummary {
            name: rt.name.clone(), namespace: rt.namespace.clone(),
            description: String::new(), status: kagenti_api::types::AgentStatus::Running,
            labels: kagenti_api::types::ResourceLabels { protocol: None, framework: None, kind: None },
            workload_type: None, created_at: None,
        });
        tokio::spawn(async move { let _ = bus.publish(&event).await; });
    }
    Json(serde_json::json!({"success": true, "message": format!("deployed {}", rt.name)}))
}

async fn start_agent(State(st): State<AppState>, Path(name): Path<String>) -> Json<serde_json::Value> {
    let rts = st.runtimes.lock().unwrap();
    match rts.iter().find(|r| r.name == name) {
        Some(rt) => match systemd::start(rt) {
            Ok(_) => Json(serde_json::json!({"success": true})),
            Err(e) => Json(serde_json::json!({"success": false, "message": e.to_string()})),
        },
        None => Json(serde_json::json!({"success": false, "message": "not found"})),
    }
}

async fn stop_agent(State(st): State<AppState>, Path(name): Path<String>) -> Json<serde_json::Value> {
    let rts = st.runtimes.lock().unwrap();
    match rts.iter().find(|r| r.name == name) {
        Some(rt) => match systemd::stop(rt) {
            Ok(_) => Json(serde_json::json!({"success": true})),
            Err(e) => Json(serde_json::json!({"success": false, "message": e.to_string()})),
        },
        None => Json(serde_json::json!({"success": false, "message": "not found"})),
    }
}

async fn agent_status(State(st): State<AppState>, Path(name): Path<String>) -> Json<serde_json::Value> {
    let rts = st.runtimes.lock().unwrap();
    match rts.iter().find(|r| r.name == name) {
        Some(rt) => match systemd::status(rt) {
            Ok(s) => Json(serde_json::json!({"name": name, "status": s})),
            Err(e) => Json(serde_json::json!({"error": e.to_string()})),
        },
        None => Json(serde_json::json!({"error": "not found"})),
    }
}

// -- reconciler --

async fn reconcile_loop(bus: Arc<EventBus>) {
    let mut rx = match bus.subscribe().await {
        Ok(rx) => rx,
        Err(e) => { eprintln!("reconciler: subscribe failed: {e}"); return; }
    };
    println!("reconciler: watching for events");
    while let Some(event) = rx.recv().await {
        match event {
            AgentEvent::Created(agent) => {
                println!("reconciler: created {}", agent.name);
                // In full impl: look up AgentRuntime, nix build, systemd install
            }
            AgentEvent::Updated(agent) => {
                println!("reconciler: updated {}", agent.name);
            }
            AgentEvent::Deleted { name, namespace } => {
                println!("reconciler: deleted {name} in {namespace}");
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Connect to RMQ (optional — daemon works without it)
    let rmq = match EventBus::connect(RMQ_URI).await {
        Ok(bus) => {
            println!("connected to RabbitMQ");
            Some(Arc::new(bus))
        }
        Err(e) => {
            eprintln!("RabbitMQ unavailable ({e}), running without events");
            None
        }
    };

    // Spawn reconciler if RMQ connected
    if let Some(bus) = &rmq {
        tokio::spawn(reconcile_loop(bus.clone()));
    }

    let state = AppState {
        agents: routes::new_store(),
        runtimes: Arc::new(Mutex::new(Vec::new())),
        rmq: rmq,
    };

    let api_routes = Router::new()
        .route("/apis/kagenti/v1/namespaces", get(routes::list_namespaces))
        .route("/apis/kagenti/v1/namespaces/:ns/agents",
            get(routes::list_agents).post(routes::create_agent))
        .route("/apis/kagenti/v1/namespaces/:ns/tools", get(routes::list_tools))
        .route("/apis/kagenti/v1/namespaces/:ns/agents/:name", delete(routes::delete_agent))
        .with_state(state.agents.clone());

    let mgmt_routes = Router::new()
        .route("/apis/kagenti/v1/deploy", post(deploy_agent))
        .route("/apis/kagenti/v1/agents/:name/start", post(start_agent))
        .route("/apis/kagenti/v1/agents/:name/stop", post(stop_agent))
        .route("/apis/kagenti/v1/agents/:name/status", get(agent_status))
        .with_state(state);

    let app = api_routes.merge(mgmt_routes);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8480").await.unwrap();
    println!("kagenti-daemon listening on 127.0.0.1:8480");
    axum::serve(listener, app).await.unwrap();
}
