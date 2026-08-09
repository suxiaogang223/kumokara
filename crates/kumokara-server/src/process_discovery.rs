//! Best-effort discovery of session working directories and known agent CLIs.

use kumokara_agent::AgentAdapterRegistry;
use kumokara_protocol::session::AgentInfo;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

pub(crate) struct ProcessContext {
    pub session_id: String,
    pub cwd: Option<PathBuf>,
    pub agent: Option<AgentInfo>,
    pub title_hint: Option<String>,
}

struct ProcessRecord {
    pid: u32,
    parent_pid: u32,
    command: String,
}

pub(crate) fn discover(
    roots: &[(String, u32)],
    adapters: &AgentAdapterRegistry,
) -> Vec<ProcessContext> {
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
                .find_map(|process| {
                    adapters
                        .detect(&process.command)
                        .map(|agent| (process.pid, agent))
                });
            let context_pid = agent_process.as_ref().map_or(*root_pid, |(pid, _)| *pid);
            let (agent, title_hint) = agent_process
                .map(|(_, agent)| {
                    let title_hint = agent.title_hint;
                    (
                        Some(AgentInfo {
                            provider: agent.provider,
                            display_name: agent.display_name,
                            icon: agent.icon,
                            status: None,
                            detail: None,
                            mode: None,
                            task_progress: None,
                        }),
                        title_hint,
                    )
                })
                .unwrap_or((None, None));

            ProcessContext {
                session_id: session_id.clone(),
                cwd: process_cwd(context_pid),
                agent,
                title_hint,
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
        let adapters = AgentAdapterRegistry::with_builtins();
        assert_eq!(
            adapters
                .detect("node /usr/local/lib/node_modules/@openai/codex/bin/codex.js")
                .unwrap()
                .provider,
            "codex"
        );
        assert_eq!(
            adapters
                .detect("/opt/homebrew/bin/claude --resume abc")
                .unwrap()
                .provider,
            "claude_code"
        );
        assert!(adapters.detect("python build.py").is_none());
    }
}
