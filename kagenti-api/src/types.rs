//! K8s-compatible API types for kagenti-native.
//! Mirrors kagenti Python models (responses.py, shipwright.py)
//! and maps to systemd/nix backends instead of k8s.

use serde::{Deserialize, Serialize, Deserializer, Serializer};

// -- FlexibleString: accepts JSON string or array, mirrors Go's FlexibleString --

fn deserialize_flexible<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<String>>, D::Error> {
    let v: Option<serde_json::Value> = Option::deserialize(d)?;
    Ok(v.map(|v| match v {
        serde_json::Value::String(s) => s.split(", ").map(String::from).collect(),
        serde_json::Value::Array(a) => a.into_iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => vec![],
    }))
}

fn serialize_flexible<S: Serializer>(v: &Option<Vec<String>>, s: S) -> Result<S::Ok, S::Error> {
    match v {
        Some(arr) => arr.serialize(s),
        None => s.serialize_none(),
    }
}

// -- Agent types --

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLabels {
    #[serde(default, deserialize_with = "deserialize_flexible", serialize_with = "serialize_flexible")]
    pub protocol: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSummary {
    pub name: String,
    pub namespace: String,
    pub description: String,
    pub status: AgentStatus,
    pub labels: ResourceLabels,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workload_type: Option<WorkloadType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentListResponse {
    pub items: Vec<AgentSummary>,
}

// -- Tool types --

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSummary {
    pub name: String,
    pub namespace: String,
    pub description: String,
    pub status: AgentStatus,
    pub labels: ResourceLabels,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workload_type: Option<WorkloadType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolListResponse {
    pub items: Vec<ToolSummary>,
}

// -- MCP types --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolsResponse {
    pub tools: Vec<MCPToolInfo>,
}

// -- Enums --

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Running,
    Stopped,
    Failed,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorkloadType {
    /// Maps to systemd service unit
    Deployment,
    /// Maps to systemd service with persistent state
    StatefulSet,
}

// -- Native extensions (systemd/nix mapping) --

/// Agent runtime spec — what kagenti-daemon uses to manage the workload.
/// Replaces k8s Deployment/Pod spec with systemd + nix.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntime {
    pub name: String,
    pub namespace: String,
    /// Nix derivation or flake ref (replaces container image)
    pub nix_package: String,
    /// Command to run
    pub command: Vec<String>,
    /// Environment variables
    #[serde(default)]
    pub env: Vec<EnvVar>,
    /// Ports to expose
    #[serde(default)]
    pub ports: Vec<Port>,
    /// Systemd unit overrides
    #[serde(skip_serializing_if = "Option::is_none")]
    pub systemd_overrides: Option<SystemdOverrides>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Port {
    pub name: String,
    pub port: u16,
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

fn default_protocol() -> String {
    "tcp".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemdOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

// -- Common responses --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceListResponse {
    pub namespaces: Vec<String>,
}
