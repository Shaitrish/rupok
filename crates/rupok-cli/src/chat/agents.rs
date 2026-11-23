//! Which ACP agents are installed on this computer, and how to start them.
//!
//! Each agent runs under the teacher's own login (Claude subscription,
//! ChatGPT, Google…); Rupok only launches it.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// An agent the chat can use.
#[derive(Clone, Debug, Serialize)]
pub struct AgentInfo {
    /// `claude`, `codex`, `opencode`, `gemini` or `custom`.
    pub id: String,
    pub label: String,
    /// Ready to start: the command was found.
    pub available: bool,
    /// The command line that speaks ACP on stdio.
    #[serde(skip)]
    pub command: Vec<String>,
    /// Shell commands that install it.
    pub install: Vec<String>,
    /// How to log in once, in a terminal.
    pub login: String,
    /// `Some(false)` when we can tell it has never been logged in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logged_in: Option<bool>,
}

/// Find `bin` on PATH, then in the usual install folders: an app launched
/// from the dock or Start menu often has a short PATH without them.
fn find(bin: &str) -> Option<PathBuf> {
    if let Ok(p) = which::which(bin) {
        return Some(p);
    }
    let home = dirs::home_dir()?;
    let mut dirs: Vec<PathBuf> = [
        ".local/bin",
        ".npm-global/bin",
        ".bun/bin",
        ".volta/bin",
        ".cargo/bin",
        ".claude/local",
        "AppData/Roaming/npm",
        ".opencode/bin",
    ]
    .iter()
    .map(|d| home.join(d))
    .collect();
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
    // nvm: every installed Node version.
    if let Ok(rd) = std::fs::read_dir(home.join(".nvm/versions/node")) {
        dirs.extend(rd.flatten().map(|e| e.path().join("bin")));
    }
    let path = std::env::join_paths(dirs).ok()?;
    which::which_in(bin, Some(path), &home).ok()
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Detect the known agents (and a custom command, first when given).
pub fn detect(custom: Option<(&str, &str)>) -> Vec<AgentInfo> {
    let mut out = Vec::new();
    if let Some((label, cmd)) = custom {
        let command: Vec<String> = cmd.split_whitespace().map(str::to_string).collect();
        out.push(AgentInfo {
            id: "custom".into(),
            label: label.to_string(),
            available: !command.is_empty(),
            command,
            install: Vec::new(),
            login: String::new(),
            logged_in: None,
        });
    }
    let npx = find("npx");
    let home = dirs::home_dir();

    // Claude Code, through its ACP adapter (Zed's, now under the ACP org).
    let claude = find("claude");
    let adapter = find("claude-agent-acp").or_else(|| find("claude-code-acp"));
    let command = match (&adapter, &claude, &npx) {
        (Some(a), _, _) => vec![path_str(a)],
        (None, Some(_), Some(n)) => vec![
            path_str(n),
            "-y".into(),
            "@agentclientprotocol/claude-agent-acp".into(),
        ],
        _ => Vec::new(),
    };
    let logged_in = if std::env::var_os("ANTHROPIC_API_KEY").is_some() {
        Some(true)
    } else if cfg!(target_os = "macos") {
        None // the login lives in the Keychain
    } else {
        home.as_ref().map(|h| {
            h.join(".claude/.credentials.json").exists() || h.join(".claude.json").exists()
        })
    };
    out.push(AgentInfo {
        id: "claude".into(),
        label: "Claude Code".into(),
        available: !command.is_empty(),
        command,
        install: vec![
            "npm install -g @anthropic-ai/claude-code".into(),
            "npm install -g @agentclientprotocol/claude-agent-acp".into(),
        ],
        login: "Open a terminal, run `claude` once and log in with your Claude account.".into(),
        logged_in: if claude.is_some() || adapter.is_some() {
            logged_in
        } else {
            None
        },
    });

    // Codex, through codex-acp.
    let codex = find("codex");
    let command = match (find("codex-acp"), &codex, &npx) {
        (Some(a), _, _) => vec![path_str(&a)],
        (None, Some(_), Some(n)) => {
            vec![path_str(n), "-y".into(), "@zed-industries/codex-acp".into()]
        }
        _ => Vec::new(),
    };
    out.push(AgentInfo {
        id: "codex".into(),
        label: "Codex".into(),
        available: !command.is_empty(),
        command,
        install: vec!["npm install -g @openai/codex @zed-industries/codex-acp".into()],
        login: "Open a terminal, run `codex` once and sign in.".into(),
        logged_in: home
            .as_ref()
            .filter(|_| codex.is_some())
            .map(|h| h.join(".codex/auth.json").exists()),
    });

    // OpenCode and Gemini CLI speak ACP themselves.
    let opencode = find("opencode");
    out.push(AgentInfo {
        id: "opencode".into(),
        label: "OpenCode".into(),
        available: opencode.is_some(),
        command: opencode
            .map(|p| vec![path_str(&p), "acp".into()])
            .unwrap_or_default(),
        install: vec!["npm install -g opencode-ai".into()],
        login: "Open a terminal and run `opencode auth login`.".into(),
        logged_in: None,
    });
    let gemini = find("gemini");
    out.push(AgentInfo {
        id: "gemini".into(),
        label: "Gemini CLI".into(),
        available: gemini.is_some(),
        command: gemini
            .map(|p| vec![path_str(&p), "--experimental-acp".into()])
            .unwrap_or_default(),
        install: vec!["npm install -g @google/gemini-cli".into()],
        login: "Open a terminal, run `gemini` once and sign in with Google.".into(),
        logged_in: None,
    });
    out
}

/// The agent to start when the teacher has not picked one: the given id,
/// else the first available.
pub fn pick<'a>(agents: &'a [AgentInfo], wanted: Option<&str>) -> Option<&'a AgentInfo> {
    match wanted {
        Some(id) => agents.iter().find(|a| a.id == id),
        None => agents.iter().find(|a| a.available),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_agent_comes_first() {
        let a = detect(Some(("Fake", "node fake.mjs --x")));
        assert_eq!(a[0].id, "custom");
        assert_eq!(a[0].command, ["node", "fake.mjs", "--x"]);
        assert_eq!(pick(&a, None).unwrap().id, "custom");
        assert!(a.iter().any(|x| x.id == "claude"));
        assert_eq!(pick(&a, Some("gemini")).unwrap().label, "Gemini CLI");
    }
}
