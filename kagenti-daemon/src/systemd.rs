//! Systemd unit management for agent workloads.

use kagenti_api::types::{AgentRuntime, SystemdOverrides};
use std::{fs, io, path::PathBuf, process::Command};

fn unit_name(rt: &AgentRuntime) -> String {
    format!("kagenti-agent-{}.service", rt.name)
}

fn unit_dir() -> PathBuf {
    let d = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config")).join("systemd/user");
    fs::create_dir_all(&d).ok();
    d
}

pub fn generate_unit(rt: &AgentRuntime) -> String {
    let ov = rt.systemd_overrides.as_ref();
    let restart = ov.and_then(|o| o.restart_policy.as_deref()).unwrap_or("on-failure");
    let mut s = format!(
        "[Unit]\nDescription=kagenti agent: {}\nAfter=network.target\n\n[Service]\nType=simple\nExecStart={}\nRestart={restart}\nRestartSec=5s\n",
        rt.name,
        rt.command.join(" "),
    );
    if let Some(wd) = ov.and_then(|o| o.working_directory.as_deref()) {
        s.push_str(&format!("WorkingDirectory={wd}\n"));
    }
    if let Some(u) = ov.and_then(|o| o.user.as_deref()) {
        s.push_str(&format!("User={u}\n"));
    }
    for e in &rt.env {
        s.push_str(&format!("Environment=\"{}={}\"\n", e.name, e.value));
    }
    s.push_str("\n[Install]\nWantedBy=default.target\n");
    s
}

pub fn install(rt: &AgentRuntime) -> io::Result<()> {
    let path = unit_dir().join(unit_name(rt));
    fs::write(&path, generate_unit(rt))?;
    systemctl(&["daemon-reload"])
}

pub fn start(rt: &AgentRuntime) -> io::Result<()> {
    systemctl(&["start", &unit_name(rt)])
}

pub fn stop(rt: &AgentRuntime) -> io::Result<()> {
    systemctl(&["stop", &unit_name(rt)])
}

pub fn status(rt: &AgentRuntime) -> io::Result<String> {
    let out = Command::new("systemctl").args(["--user", "is-active", &unit_name(rt)]).output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn remove(rt: &AgentRuntime) -> io::Result<()> {
    systemctl(&["stop", &unit_name(rt)]).ok();
    systemctl(&["disable", &unit_name(rt)]).ok();
    fs::remove_file(unit_dir().join(unit_name(rt))).ok();
    systemctl(&["daemon-reload"])
}

fn systemctl(args: &[&str]) -> io::Result<()> {
    let mut cmd = Command::new("systemctl");
    cmd.arg("--user").args(args);
    let out = cmd.output()?;
    if out.status.success() { Ok(()) } else {
        Err(io::Error::new(io::ErrorKind::Other, String::from_utf8_lossy(&out.stderr).to_string()))
    }
}
