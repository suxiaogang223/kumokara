//! Extensible adapters for coding agents running inside a terminal session.

use std::sync::Arc;

/// Process information exposed to an adapter during best-effort discovery.
pub struct AgentProcess<'a> {
    pub command: &'a str,
}

/// Stable UI identity and protocol aliases owned by an adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentManifest {
    pub provider: &'static str,
    pub display_name: &'static str,
    pub icon: &'static str,
    pub protocol_names: &'static [&'static str],
}

/// Adapter output consumed by the Session runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentMatch {
    pub provider: String,
    pub display_name: String,
    pub icon: String,
    pub title_hint: Option<String>,
}

/// Provider plugin contract.
///
/// Implementations may inspect a process command today. Future hook/plugin
/// installers can live behind the same provider boundary without changing the
/// Session or UI models.
pub trait AgentAdapter: Send + Sync {
    fn manifest(&self) -> AgentManifest;
    fn detect(&self, process: &AgentProcess<'_>) -> bool;

    fn title_hint(&self, _process: &AgentProcess<'_>) -> Option<String> {
        None
    }
}

/// Ordered adapter registry. Earlier registrations win when commands overlap.
#[derive(Default)]
pub struct AgentAdapterRegistry {
    adapters: Vec<Arc<dyn AgentAdapter>>,
}

impl AgentAdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        for adapter in builtins() {
            registry.register_arc(adapter);
        }
        registry
    }

    pub fn register<A: AgentAdapter + 'static>(&mut self, adapter: A) {
        self.adapters.push(Arc::new(adapter));
    }

    pub fn register_arc(&mut self, adapter: Arc<dyn AgentAdapter>) {
        self.adapters.push(adapter);
    }

    pub fn detect(&self, command: &str) -> Option<AgentMatch> {
        let process = AgentProcess { command };
        self.adapters
            .iter()
            .find(|adapter| adapter.detect(&process))
            .map(|adapter| to_match(adapter.as_ref(), &process))
    }

    /// Resolve an OSC 26 `CodeAgent` token to its installed presentation.
    pub fn resolve(&self, code_agent: &str) -> Option<AgentMatch> {
        self.adapters
            .iter()
            .find(|adapter| {
                let manifest = adapter.manifest();
                manifest.provider.eq_ignore_ascii_case(code_agent)
                    || manifest
                        .protocol_names
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(code_agent))
            })
            .map(|adapter| to_match(adapter.as_ref(), &AgentProcess { command: "" }))
    }
}

fn to_match(adapter: &dyn AgentAdapter, process: &AgentProcess<'_>) -> AgentMatch {
    let manifest = adapter.manifest();
    AgentMatch {
        provider: manifest.provider.to_string(),
        display_name: manifest.display_name.to_string(),
        icon: manifest.icon.to_string(),
        title_hint: adapter.title_hint(process),
    }
}

#[derive(Clone, Copy)]
enum ProcessMatcher {
    AnyToken(&'static [&'static str]),
    Executable(&'static [&'static str]),
}

struct BuiltinAdapter {
    manifest: AgentManifest,
    matcher: ProcessMatcher,
}

impl AgentAdapter for BuiltinAdapter {
    fn manifest(&self) -> AgentManifest {
        self.manifest
    }

    fn detect(&self, process: &AgentProcess<'_>) -> bool {
        match self.matcher {
            ProcessMatcher::AnyToken(names) => command_tokens(process.command)
                .any(|token| names.iter().any(|name| token.eq_ignore_ascii_case(name))),
            ProcessMatcher::Executable(names) => executable_name(process.command)
                .is_some_and(|token| names.iter().any(|name| token.eq_ignore_ascii_case(name))),
        }
    }
}

fn command_tokens(command: &str) -> impl Iterator<Item = &str> {
    command.split(|character: char| {
        !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
    })
}

fn executable_name(command: &str) -> Option<&str> {
    command
        .split_whitespace()
        .next()
        .and_then(|value| value.rsplit('/').next())
}

fn builtins() -> Vec<Arc<dyn AgentAdapter>> {
    vec![
        builtin(
            "claude_code",
            "Claude Code",
            "✦",
            &["claude", "claude-code"],
            ProcessMatcher::AnyToken(&["claude", "claude-code"]),
        ),
        builtin(
            "codex",
            "Codex",
            "◇",
            &["codex"],
            ProcessMatcher::AnyToken(&["codex"]),
        ),
        builtin(
            "opencode",
            "OpenCode",
            "◉",
            &["opencode"],
            ProcessMatcher::AnyToken(&["opencode"]),
        ),
        builtin(
            "kimi_code",
            "Kimi Code",
            "◐",
            &["kimi", "kimi-code"],
            ProcessMatcher::AnyToken(&["kimi", "kimi-code", "kimi-cli"]),
        ),
        builtin(
            "mimo_code",
            "Mimo Code",
            "◒",
            &["mimo", "mimo-code"],
            ProcessMatcher::AnyToken(&["mimo", "mimo-code", "mimo-cli"]),
        ),
        builtin(
            "pi",
            "Pi",
            "π",
            &["pi"],
            ProcessMatcher::Executable(&["pi"]),
        ),
        builtin(
            "omp",
            "omp",
            "✳",
            &["omp"],
            ProcessMatcher::Executable(&["omp"]),
        ),
        // `agent` is intentionally executable-only because it is too generic
        // to match as an arbitrary command-line token.
        builtin(
            "cursor",
            "Cursor",
            "◆",
            &["cursor", "agent"],
            ProcessMatcher::Executable(&["agent"]),
        ),
    ]
}

fn builtin(
    provider: &'static str,
    display_name: &'static str,
    icon: &'static str,
    protocol_names: &'static [&'static str],
    matcher: ProcessMatcher,
) -> Arc<dyn AgentAdapter> {
    Arc::new(BuiltinAdapter {
        manifest: AgentManifest {
            provider,
            display_name,
            icon,
            protocol_names,
        },
        matcher,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestAdapter;

    impl AgentAdapter for TestAdapter {
        fn manifest(&self) -> AgentManifest {
            AgentManifest {
                provider: "test_agent",
                display_name: "Test Agent",
                icon: "T",
                protocol_names: &["test"],
            }
        }

        fn detect(&self, process: &AgentProcess<'_>) -> bool {
            process.command.contains("test-agent")
        }

        fn title_hint(&self, _process: &AgentProcess<'_>) -> Option<String> {
            Some("Adapter title".to_string())
        }
    }

    #[test]
    fn builtins_detect_processes_and_resolve_protocol_aliases() {
        let registry = AgentAdapterRegistry::with_builtins();
        assert_eq!(
            registry
                .detect("node /usr/local/lib/node_modules/@openai/codex/bin/codex.js")
                .unwrap()
                .display_name,
            "Codex"
        );
        assert_eq!(registry.resolve("claude").unwrap().provider, "claude_code");
        assert!(registry.detect("python build.py").is_none());
        assert!(registry.detect("tool --agent worker").is_none());
    }

    #[test]
    fn custom_adapters_register_without_server_changes() {
        let mut registry = AgentAdapterRegistry::new();
        registry.register(TestAdapter);
        let detected = registry.detect("/opt/bin/test-agent run").unwrap();
        assert_eq!(detected.provider, "test_agent");
        assert_eq!(detected.title_hint.as_deref(), Some("Adapter title"));
        assert_eq!(registry.resolve("test").unwrap().icon, "T");
    }
}
