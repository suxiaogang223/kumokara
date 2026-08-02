//! Best-effort discovery of session working directories and known agent CLIs.

use kumokara_protocol::session::AgentInfo;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

pub(crate) struct ProcessContext {
    pub session_id: String,
    pub cwd: Option<PathBuf>,
    pub agent: Option<AgentInfo>,
}

struct ProcessRecord {
    pid: u32,
    parent_pid: u32,
    command: String,
}

pub(crate) fn discover(roots: &[(String, u32)]) -> Vec<ProcessContext> {
    let records = snapshot();
    roots
        .iter()
        .map(|(session_id, root_pid)| {
            let mut descendants = HashSet::from([*root_pid]);
            loop {
                let before = descendants.len();
                for process in &records {
                    if descendants.contains(&process.parent_pid) {
                        descendants.insert(process.pid);
                    }
                }
                if descendants.len() == before {
                    break;
                }
            }

            let agent_process = records
                .iter()
                .filter(|process| process.pid != *root_pid && descendants.contains(&process.pid))
                .find_map(|process| detect_agent(&process.command).map(|name| (process.pid, name)));
            let context_pid = agent_process.as_ref().map_or(*root_pid, |(pid, _)| *pid);

            ProcessContext {
                session_id: session_id.clone(),
                cwd: process_cwd(context_pid),
                agent: agent_process.map(|(_, provider)| AgentInfo { provider }),
            }
        })
        .collect()
}

fn snapshot() -> Vec<ProcessRecord> {
    let Ok(output) = Command::new("ps")
        .args(["-axo", "pid=,ppid=,command="])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some(ProcessRecord {
                pid: fields.next()?.parse().ok()?,
                parent_pid: fields.next()?.parse().ok()?,
                command: fields.collect::<Vec<_>>().join(" "),
            })
        })
        .collect()
}

fn detect_agent(command: &str) -> Option<String> {
    command
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
        })
        .find_map(|part| match part.to_ascii_lowercase().as_str() {
            "claude" | "claude-code" => Some("claude_code".to_string()),
            "codex" => Some("codex".to_string()),
            "opencode" => Some("opencode".to_string()),
            "kimi" | "kimi-code" | "kimi-cli" => Some("kimi_code".to_string()),
            "mimo" | "mimo-code" | "mimo-cli" => Some("mimo_code".to_string()),
            _ => None,
        })
}

#[cfg(target_os = "linux")]
fn process_cwd(pid: u32) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

#[cfg(target_os = "macos")]
fn process_cwd(pid: u32) -> Option<PathBuf> {
    let output = Command::new("lsof")
        .args(["-a", "-d", "cwd", "-Fn", "-p", &pid.to_string()])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix('n').map(PathBuf::from))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn process_cwd(_pid: u32) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_supported_agents_without_matching_unrelated_commands() {
        assert_eq!(
            detect_agent("node /usr/local/lib/node_modules/@openai/codex/bin/codex.js"),
            Some("codex".to_string())
        );
        assert_eq!(
            detect_agent("/opt/homebrew/bin/claude --resume abc"),
            Some("claude_code".to_string())
        );
        assert_eq!(detect_agent("python build.py"), None);
    }
}
