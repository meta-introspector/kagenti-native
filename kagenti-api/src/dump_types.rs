//! Instantiate all kagenti API types and publish as DASL/CBOR shards
//! using erdfa-publish (DA51-tagged CBOR, content-addressed, tar bundle).

use erdfa_publish::*;
use kagenti_api::types::*;
use std::fs;

fn agent_shard(agent: &AgentSummary) -> Shard {
    let json = serde_json::to_string_pretty(agent).unwrap();
    Shard::new(
        &agent.name,
        Component::KeyValue {
            pairs: vec![
                ("name".into(), agent.name.clone()),
                ("namespace".into(), agent.namespace.clone()),
                ("description".into(), agent.description.clone()),
                ("status".into(), serde_json::to_string(&agent.status).unwrap()),
                ("json".into(), json),
            ],
        },
    )
    .with_tags(vec!["kagenti".into(), "agent".into()])
}

fn tool_shard(tool: &ToolSummary) -> Shard {
    let json = serde_json::to_string_pretty(tool).unwrap();
    Shard::new(
        &tool.name,
        Component::KeyValue {
            pairs: vec![
                ("name".into(), tool.name.clone()),
                ("namespace".into(), tool.namespace.clone()),
                ("description".into(), tool.description.clone()),
                ("json".into(), json),
            ],
        },
    )
    .with_tags(vec!["kagenti".into(), "tool".into()])
}

fn runtime_shard(rt: &AgentRuntime) -> Shard {
    let json = serde_json::to_string_pretty(rt).unwrap();
    Shard::new(
        &format!("{}-runtime", rt.name),
        Component::Code {
            language: "json".into(),
            source: json,
        },
    )
    .with_tags(vec!["kagenti".into(), "runtime".into(), "nix".into()])
}

fn mcp_shard(tools: &MCPToolsResponse) -> Shard {
    let json = serde_json::to_string_pretty(tools).unwrap();
    Shard::new(
        "mcp-tools",
        Component::Code {
            language: "json".into(),
            source: json,
        },
    )
    .with_tags(vec!["kagenti".into(), "mcp".into()])
}

fn rdfa_triples(agent: &AgentSummary) -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("kagenti:agent", "rdf:type", "kagenti:AgentSummary"),
        ("kagenti:agent", "kagenti:status", "kagenti:Running"),
        ("kagenti:agent", "kagenti:protocol", "a2a"),
        ("kagenti:agent", "kagenti:framework", "LangGraph"),
    ]
}

fn main() {
    let agent = AgentSummary {
        name: "git-issue-agent".into(),
        namespace: "default".into(),
        description: "Resolves GitHub issues via LLM".into(),
        status: AgentStatus::Running,
        labels: ResourceLabels {
            protocol: Some(vec!["a2a".into(), "mcp".into()]),
            framework: Some("LangGraph".into()),
            kind: Some("agent".into()),
        },
        workload_type: Some(WorkloadType::Deployment),
        created_at: Some("2026-03-18T13:00:00Z".into()),
    };

    let tool = ToolSummary {
        name: "web-search-tool".into(),
        namespace: "default".into(),
        description: "Web search via MCP".into(),
        status: AgentStatus::Running,
        labels: ResourceLabels {
            protocol: Some(vec!["mcp".into()]),
            framework: None,
            kind: Some("tool".into()),
        },
        created_at: None,
        workload_type: Some(WorkloadType::StatefulSet),
    };

    let runtime = AgentRuntime {
        name: "git-issue-agent".into(),
        namespace: "default".into(),
        nix_package: "github:kagenti/agent-examples#git-issue-agent".into(),
        command: vec!["python".into(), "-m".into(), "agent".into()],
        env: vec![
            EnvVar { name: "MODEL".into(), value: "gpt-4".into() },
        ],
        ports: vec![Port { name: "http".into(), port: 8080, protocol: "tcp".into() }],
        systemd_overrides: Some(SystemdOverrides {
            restart_policy: Some("on-failure".into()),
            working_directory: Some("/var/lib/kagenti/git-issue-agent".into()),
            user: Some("kagenti".into()),
        }),
    };

    let mcp = MCPToolsResponse {
        tools: vec![MCPToolInfo {
            name: "search".into(),
            description: Some("Search the web".into()),
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"]
            })),
        }],
    };

    // Build shards
    let shards = vec![
        agent_shard(&agent),
        tool_shard(&tool),
        runtime_shard(&runtime),
        mcp_shard(&mcp),
    ];

    // Build manifest
    let mut set = ShardSet::from_shards("kagenti-native-types", &shards);

    // Dump individual CBOR + JSON
    let dir = "dasl-dump";
    fs::create_dir_all(dir).unwrap();
    for shard in &shards {
        let cbor = shard.to_cbor();
        fs::write(format!("{dir}/{}.cbor", shard.id), &cbor).unwrap();
        println!("{}: cid={} cbor={}B", shard.id, shard.cid, cbor.len());
    }

    // RDFa triples (wire-compatible with eRDFa WASM)
    let triples = rdfa_triples(&agent);
    let (triple_cid, triple_cbor) = triple_shard("kagenti-agent-triples", &triples);
    fs::write(format!("{dir}/agent-triples.cbor"), &triple_cbor).unwrap();
    println!("triples: cid={triple_cid} cbor={}B", triple_cbor.len());

    // Tar bundle
    let tar_path = format!("{dir}/kagenti-types.tar");
    let tar = fs::File::create(&tar_path).unwrap();
    set.to_tar(&shards, tar).unwrap();
    let tar_size = fs::metadata(&tar_path).unwrap().len();
    println!("\n✅ {tar_path}: {tar_size}B ({} shards)", set.shards.len());

    // Manifest CBOR
    let manifest = set.to_cbor();
    fs::write(format!("{dir}/manifest.cbor"), &manifest).unwrap();
    println!("manifest: {}B", manifest.len());
}
