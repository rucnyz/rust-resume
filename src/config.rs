use std::path::PathBuf;

pub struct AgentConfig {
    pub color: &'static str,
    pub badge: &'static str,
}

pub const AGENTS: &[(&str, AgentConfig)] = &[
    (
        "claude",
        AgentConfig {
            color: "#E87B35",
            badge: "claude",
        },
    ),
    (
        "codex",
        AgentConfig {
            color: "#00A67E",
            badge: "codex",
        },
    ),
    (
        "opencode",
        AgentConfig {
            color: "#CFCECD",
            badge: "opencode",
        },
    ),
    (
        "vibe",
        AgentConfig {
            color: "#FF6B35",
            badge: "vibe",
        },
    ),
    (
        "crush",
        AgentConfig {
            color: "#6B51FF",
            badge: "crush",
        },
    ),
    (
        "copilot-cli",
        AgentConfig {
            color: "#9CA3AF",
            badge: "copilot",
        },
    ),
    (
        "copilot-vscode",
        AgentConfig {
            color: "#007ACC",
            badge: "vscode",
        },
    ),
];

pub const SCHEMA_VERSION: u32 = 20;

fn home_dir() -> PathBuf {
    dirs::home_dir().expect("could not determine home directory")
}

pub fn claude_dir() -> PathBuf {
    home_dir().join(".claude").join("projects")
}

pub fn codex_dir() -> PathBuf {
    home_dir().join(".codex").join("sessions")
}

pub fn copilot_dir() -> PathBuf {
    home_dir().join(".copilot").join("session-state")
}

pub fn opencode_dir() -> PathBuf {
    home_dir().join(".local/share/opencode")
}

pub fn opencode_db() -> PathBuf {
    home_dir().join(".local/share/opencode/opencode.db")
}

pub fn vibe_dir() -> PathBuf {
    home_dir().join(".vibe/logs/session")
}

pub fn crush_projects_file() -> PathBuf {
    home_dir().join(".local/share/crush/projects.json")
}

pub fn cache_dir() -> PathBuf {
    home_dir().join(".cache/fast-resume")
}

pub fn index_dir() -> PathBuf {
    cache_dir().join("tantivy_index_rs")
}

pub fn log_file() -> PathBuf {
    cache_dir().join("parse-errors.log")
}

pub fn get_agent_config(name: &str) -> Option<&'static AgentConfig> {
    AGENTS.iter().find(|(n, _)| *n == name).map(|(_, c)| c)
}
