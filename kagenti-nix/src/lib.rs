//! kagenti-nix: Generate nix flakes from AgentRuntime specs
use kagenti_api::types::AgentRuntime;
use std::{fs, io, path::Path, process::Command};

/// Generate a flake.nix for an agent workload.
pub fn generate_flake(rt: &AgentRuntime) -> String {
    let env_lines: String = rt.env.iter()
        .map(|e| format!("          export {}=\"{}\";", e.name, e.value))
        .collect::<Vec<_>>()
        .join("\n");

    let cmd = rt.command.join(" ");

    let (inputs, ri) = if rt.nix_package.is_empty() {
        (String::new(), String::new())
    } else {
        (
            format!("    agent-pkg.url = \"{}\";\n", rt.nix_package),
            "runtimeInputs = [ agent-pkg.packages.${system}.default ];".to_string(),
        )
    };

    let mut s = String::new();
    s.push_str("{\n  inputs = {\n    nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";\n");
    s.push_str(&inputs);
    s.push_str("  };\n\n");
    s.push_str("  outputs = { self, nixpkgs, ... }@inputs: let\n");
    s.push_str("    systems = [ \"x86_64-linux\" \"aarch64-linux\" ];\n");
    s.push_str("    forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);\n");
    s.push_str("  in {\n    packages = forAllSystems (system: let\n");
    s.push_str("      pkgs = nixpkgs.legacyPackages.${system};\n");
    s.push_str("    in {\n      default = pkgs.writeShellApplication {\n");
    s.push_str(&format!("        name = \"{}\";\n", rt.name));
    if !ri.is_empty() {
        s.push_str(&format!("        {ri}\n"));
    }
    s.push_str("        text = ''\n");
    if !env_lines.is_empty() {
        s.push_str(&env_lines);
        s.push('\n');
    }
    s.push_str(&format!("          exec {cmd}\n"));
    s.push_str("        '';\n      };\n    });\n  };\n}\n");
    s
}

/// Write flake.nix for an agent into a directory and optionally build it.
pub fn build_agent(rt: &AgentRuntime, out_dir: &Path) -> io::Result<String> {
    fs::create_dir_all(out_dir)?;
    fs::write(out_dir.join("flake.nix"), generate_flake(rt))?;

    // Init git if needed (nix flake requires it)
    if !out_dir.join(".git").exists() {
        Command::new("git").args(["init"]).current_dir(out_dir).output()?;
        Command::new("git").args(["add", "flake.nix"]).current_dir(out_dir).output()?;
    } else {
        Command::new("git").args(["add", "flake.nix"]).current_dir(out_dir).output()?;
    }

    let out = Command::new("nix").args(["build", "--no-link", "--print-out-paths"])
        .current_dir(out_dir).output()?;

    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, String::from_utf8_lossy(&out.stderr).to_string()))
    }
}
