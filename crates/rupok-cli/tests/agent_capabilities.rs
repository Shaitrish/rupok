//! Rupok is agent-first (AGENTS.md, rule 8): every setting, dock button and
//! live operation must have an agent path or be marked "teacher only" with a
//! reason in docs/agent-capabilities.md, and the settings an agent may
//! change must match the board's allowlist (apps/board/src/agentSettings.ts).
//! This test keeps the three in step. It reads the sources as text.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Every `"…"` string after `marker` up to the first `end`.
fn quoted_between(src: &str, marker: &str, end: &str) -> Vec<String> {
    let start = src.find(marker).unwrap_or_else(|| panic!("no `{marker}`")) + marker.len();
    let rest = &src[start..];
    let body = &rest[..rest.find(end).unwrap_or(rest.len())];
    body.split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.to_string())
        .collect()
}

/// Registry ids: `id: "x.y"` in settings/defs (dotted ids; keyboard list skipped).
fn registry_ids() -> BTreeSet<String> {
    let dir = root().join("apps/board/src/settings/defs");
    let mut out = BTreeSet::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let src = std::fs::read_to_string(&p).unwrap();
        for part in src.split("id: \"").skip(1) {
            let id = &part[..part.find('"').unwrap()];
            if id.contains('.') && !id.starts_with("keyboard.") && !id.contains('$') {
                out.insert(id.to_string());
            }
        }
    }
    out
}

/// `| \`id\` | who | how |` rows of the section titled `heading`.
fn rows(doc: &str, heading: &str) -> Vec<(String, String, String)> {
    let start = doc
        .find(heading)
        .unwrap_or_else(|| panic!("no section {heading}"));
    let body = &doc[start + heading.len()..];
    let body = &body[..body.find("\n## ").unwrap_or(body.len())];
    body.lines()
        .filter(|l| l.starts_with("| `"))
        .map(|l| {
            let cells: Vec<String> = l
                .trim_matches('|')
                .split(" | ")
                .map(|c| c.trim().to_string())
                .collect();
            let id = cells[0].trim_matches('`').to_string();
            (
                id,
                cells.get(1).cloned().unwrap_or_default(),
                cells.get(2).cloned().unwrap_or_default(),
            )
        })
        .collect()
}

#[test]
fn every_setting_has_an_agent_path_or_a_reason() {
    let doc = read("docs/agent-capabilities.md");
    let registry = registry_ids();
    assert!(registry.len() > 40, "found {} settings", registry.len());
    let table = rows(&doc, "## Settings");
    let listed: BTreeSet<String> = table.iter().map(|r| r.0.clone()).collect();
    let missing: Vec<&String> = registry.difference(&listed).collect();
    assert!(
        missing.is_empty(),
        "settings with no row in docs/agent-capabilities.md (add them as agent or teacher only, with a reason): {missing:?}"
    );
    let stale: Vec<&String> = listed.difference(&registry).collect();
    assert!(
        stale.is_empty(),
        "rows for settings that no longer exist: {stale:?}"
    );
    for (id, who, how) in &table {
        assert!(
            who == "agent" || who == "teacher only",
            "{id}: who must be `agent` or `teacher only`, got `{who}`"
        );
        assert!(how.len() > 8, "{id}: say how, or why it is teacher only");
    }
    let by_set_setting: BTreeSet<String> = table
        .iter()
        .filter(|(_, who, how)| who == "agent" && how.starts_with("`set_setting`"))
        .map(|r| r.0.clone())
        .collect();
    let allow: BTreeSet<String> = quoted_between(
        &read("apps/board/src/agentSettings.ts"),
        "AGENT_SETTINGS: readonly string[] = [",
        "];",
    )
    .into_iter()
    .collect();
    assert_eq!(
        by_set_setting, allow,
        "the settings marked `set_setting` in docs/agent-capabilities.md must be exactly AGENT_SETTINGS in agentSettings.ts"
    );
    for forbidden in [
        "ai.claudeKey",
        "ai.allowEdit",
        "ai.chatPermissions",
        "general.cloudAccount",
    ] {
        assert!(
            !allow.contains(forbidden),
            "{forbidden} must stay teacher only"
        );
    }
}

#[test]
fn every_dock_button_is_listed() {
    let doc = read("docs/agent-capabilities.md");
    let modes = read("apps/board/src/modes.ts");
    let start = modes.find("export const TOOLBAR_ITEMS").unwrap();
    let block = &modes[start..start + modes[start..].find("];").unwrap()];
    let mut ids: BTreeSet<String> = block
        .split("{ id: \"")
        .skip(1)
        .map(|p| p[..p.find('"').unwrap()].to_string())
        .collect();
    ids.insert("shapes".into());
    let listed: BTreeSet<String> = rows(&doc, "## Dock buttons")
        .into_iter()
        .map(|r| r.0)
        .collect();
    let missing: Vec<&String> = ids.difference(&listed).collect();
    assert!(
        missing.is_empty(),
        "dock buttons missing from docs/agent-capabilities.md: {missing:?}"
    );
}

#[test]
fn every_live_operation_is_in_the_matrix() {
    let doc = read("docs/agent-capabilities.md");
    let op = read("crates/rupok-live/src/op.rs");
    let start = op.find("pub fn name(&self)").unwrap();
    let body = &op[start..start + op[start..].find("pub fn access").unwrap()];
    let names: Vec<&str> = body
        .split("=> \"")
        .skip(1)
        .map(|p| &p[..p.find('"').unwrap()])
        .collect();
    assert!(names.len() > 60, "found {} ops", names.len());
    let section = &doc[doc.find("## Live operations").unwrap()..];
    for n in names {
        assert!(
            section.contains(&format!("`{n}`")),
            "live op `{n}` is not in the Live operations table of docs/agent-capabilities.md"
        );
    }
}
