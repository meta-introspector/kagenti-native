//! Axum route handlers for the k8s-compatible API.

use axum::{extract::{Path, State}, Json};
use crate::types::*;
use std::sync::{Arc, Mutex};

pub type Store = Arc<Mutex<Vec<AgentSummary>>>;

pub fn new_store() -> Store {
    Arc::new(Mutex::new(Vec::new()))
}

/// POST /apis/kagenti/v1/namespaces/:ns/agents
pub async fn create_agent(
    State(store): State<Store>,
    Path(ns): Path<String>,
    Json(mut agent): Json<AgentSummary>,
) -> Json<AgentSummary> {
    agent.namespace = ns;
    store.lock().unwrap().push(agent.clone());
    Json(agent)
}

/// GET /apis/kagenti/v1/namespaces/:ns/agents
pub async fn list_agents(
    State(store): State<Store>,
    Path(ns): Path<String>,
) -> Json<AgentListResponse> {
    let items: Vec<_> = store.lock().unwrap()
        .iter().filter(|a| a.namespace == ns).cloned().collect();
    Json(AgentListResponse { items })
}

/// GET /apis/kagenti/v1/namespaces
pub async fn list_namespaces() -> Json<NamespaceListResponse> {
    Json(NamespaceListResponse { namespaces: vec!["default".into()] })
}

/// GET /apis/kagenti/v1/namespaces/:ns/tools
pub async fn list_tools(Path(_ns): Path<String>) -> Json<ToolListResponse> {
    Json(ToolListResponse { items: vec![] })
}

/// DELETE /apis/kagenti/v1/namespaces/:ns/agents/:name
pub async fn delete_agent(Path((ns, name)): Path<(String, String)>) -> Json<DeleteResponse> {
    Json(DeleteResponse {
        success: true,
        message: format!("deleted agent {name} in {ns}"),
    })
}
