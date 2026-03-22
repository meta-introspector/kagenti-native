fn main() {
    let rt = kagenti_api::types::AgentRuntime {
        name: "hello-agent".into(),
        namespace: "default".into(),
        nix_package: String::new(),
        command: vec!["echo".into(), "hello from kagenti".into()],
        env: vec![kagenti_api::types::EnvVar { name: "AGENT_MODE".into(), value: "production".into() }],
        ports: vec![],
        systemd_overrides: None,
    };
    println!("{}", kagenti_nix::generate_flake(&rt));
    let dir = std::path::Path::new("/tmp/kagenti-nix-test");
    match kagenti_nix::build_agent(&rt, dir) {
        Ok(path) => println!("✅ Built: {path}"),
        Err(e) => println!("❌ {e}"),
    }
}
