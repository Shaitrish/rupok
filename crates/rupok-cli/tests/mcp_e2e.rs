//! Spawns the real `rupok mcp` binary and talks JSON-RPC to it over stdio.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(30);

struct Client {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    next_id: u64,
}

impl Client {
    fn spawn(workspace: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rupok"))
            .arg("mcp")
            .arg("--workspace")
            .arg(workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn rupok mcp");
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            stdin,
            lines: rx,
            next_id: 1,
        }
    }

    fn send(&mut self, msg: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
    }

    fn notify(&mut self, method: &str) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method }));
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let msg = self.request_msg(method, params);
        assert!(msg.get("error").is_none(), "{method} failed: {msg}");
        msg["result"].clone()
    }

    /// The full JSON-RPC response (result or error).
    fn request_msg(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let line = self
                .lines
                .recv_timeout(TIMEOUT)
                .unwrap_or_else(|_| panic!("no response to {method}"));
            let msg: Value = serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("bad JSON from server: {e}: {line}"));
            if msg.get("id") == Some(&json!(id)) {
                return msg;
            }
            // Ignore notifications / server requests.
        }
    }

    fn call(&mut self, tool: &str, args: Value) -> Value {
        self.request("tools/call", json!({ "name": tool, "arguments": args }))
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn structured(result: &Value) -> Value {
    if let Some(s) = result.get("structuredContent") {
        return s.clone();
    }
    let text = result["content"][0]["text"].as_str().expect("text content");
    serde_json::from_str(text).unwrap_or(Value::String(text.to_string()))
}

#[test]
fn mcp_server_over_stdio() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Client::spawn(dir.path());

    let init = c.request(
        "initialize",
        json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "e2e-test", "version": "0.0.1" }
        }),
    );
    assert_eq!(init["serverInfo"]["name"], "rupok");
    assert!(init["capabilities"]["tools"].is_object(), "{init}");
    let instructions = init["instructions"].as_str().unwrap();
    for needle in ["read_object_spec", "1920×1080", "24px", "rupoklib://"] {
        assert!(
            instructions.contains(needle),
            "instructions lack {needle}: {instructions}"
        );
    }
    c.notify("notifications/initialized");

    let list = c.request("tools/list", json!({}));
    let tools = list["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for expected in [
        "list_lessons",
        "create_lesson",
        "get_lesson_outline",
        "add_page",
        "add_smart_object",
        "update_smart_object",
        "add_text",
        "delete_item",
        "replace_strokes_with_text",
        "set_page_background",
        "validate_smart_object",
        "read_object_spec",
    ] {
        assert!(
            names.contains(&expected),
            "missing tool {expected}: {names:?}"
        );
    }
    let add_obj = tools
        .iter()
        .find(|t| t["name"] == "add_smart_object")
        .unwrap();
    assert_eq!(add_obj["inputSchema"]["type"], "object");
    assert!(add_obj["inputSchema"]["properties"]["html"]["description"].is_string());
    assert!(
        add_obj["description"]
            .as_str()
            .unwrap()
            .contains("read_object_spec")
    );

    // create_lesson
    let r = c.call(
        "create_lesson",
        json!({ "path": "units/plants.rpk", "title": "How plants grow", "pages": 2 }),
    );
    assert_ne!(r["isError"], true, "{r}");
    let created = structured(&r);
    assert_eq!(created["path"], "units/plants.rpk");
    assert_eq!(created["page_ids"].as_array().unwrap().len(), 2);
    assert!(dir.path().join("units/plants.rpk").is_file());

    // add_smart_object (valid)
    let html = r#"<!doctype html><html><head><title>Photosynthesis</title>
<style>body{margin:0;font:32px system-ui;display:grid;place-items:center;height:100vh}</style></head>
<body><h1 id="t">Sunlight</h1>
<script>const s=["Sunlight","Water","Sugar"];rupok.steps.onChange(i=>{t.textContent=s[Math.min(i,2)]});</script>
</body></html>"#;
    let r = c.call(
        "add_smart_object",
        json!({ "path": "units/plants.rpk", "page": 1, "html": html, "title": "Photosynthesis", "steps": 2,
                "width": 1920, "height": 1080 }),
    );
    assert_ne!(r["isError"], true, "{r}");
    let added = structured(&r);
    let bundle_id = added["bundle_id"].as_str().unwrap().to_string();
    assert!(bundle_id.starts_with("photosynthesis-"), "{added}");
    assert_eq!(added["geometry"]["x"], 0.0);
    assert!(added["item_id"].is_string());

    // add_smart_object (invalid: CDN script) → tool error with diagnostics, nothing saved
    let before = std::fs::read(dir.path().join("units/plants.rpk")).unwrap();
    let r = c.call(
        "add_smart_object",
        json!({ "path": "units/plants.rpk", "page": 1, "title": "Chart",
                "html": "<script src=\"https://cdn.jsdelivr.net/npm/chart.js\"></script>" }),
    );
    assert_eq!(r["isError"], true, "{r}");
    let err = structured(&r);
    assert_eq!(err["saved"], false);
    assert_eq!(err["diagnostics"][0]["code"], "network", "{err}");
    assert_eq!(
        std::fs::read(dir.path().join("units/plants.rpk")).unwrap(),
        before
    );

    // outline reflects the object
    let outline = structured(&c.call("get_lesson_outline", json!({ "path": "units/plants.rpk" })));
    assert_eq!(outline["page_count"], 2);
    let items = outline["pages"][0]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["bundle_id"], bundle_id.as_str());
    assert_eq!(items[0]["steps"], 2);

    // sandbox: traversal is refused as a tool error
    let r = c.call(
        "create_lesson",
        json!({ "path": "../escape.rpk", "title": "x" }),
    );
    assert_eq!(r["isError"], true);
    assert!(!dir.path().parent().unwrap().join("escape.rpk").exists());

    // unknown argument names are rejected (helps agents notice typos)
    let m = c.request_msg(
        "tools/call",
        json!({ "name": "add_text", "arguments": { "path": "units/plants.rpk", "page": 1, "txt": "x", "x": 0, "y": 0 } }),
    );
    assert!(
        m.get("error").is_some() || m["result"]["isError"] == true,
        "{m}"
    );

    // ------------------------------------------------ agent control on a file
    for expected in [
        "list_pages",
        "move_pages",
        "delete_pages",
        "duplicate_pages",
        "select_items",
        "move_items",
        "align_items",
        "restyle_items",
        "scale_items",
        "group_items",
        "bring_to_front",
        "send_to_back",
        "delete_items",
        "draw_stroke",
        "update_text",
        "list_stamps",
        "create_stamp",
        "update_stamp",
        "delete_stamp",
        "add_stamp",
        "set_mode",
        "zoom_to",
        "fit_page",
        "start_replay",
        "celebrate",
        "export_pdf",
        "library_list",
        "library_open",
        "get_settings",
        "set_setting",
        "get_dock_layout",
        "set_dock_layout",
        "list_dock_items",
        "apply_dock_preset",
    ] {
        assert!(names.contains(&expected), "missing tool {expected}");
    }
    let f = "units/plants.rpk";
    let r = structured(&c.call(
        "draw_stroke",
        json!({ "path": f, "page": 2, "points": [[100, 100], [400, 300]], "brush": "chalk" }),
    ));
    let stroke = r["item_ids"][0].as_str().unwrap().to_string();
    let t = structured(&c.call(
        "add_text",
        json!({ "path": f, "page": 2, "text": "Leaf", "x": 600, "y": 100, "size": 48 }),
    ));
    let text = t["item_id"].as_str().unwrap().to_string();
    let r = c.call(
        "restyle_items",
        json!({ "path": f, "page": 2, "item_ids": [stroke, text], "color": "#30a46c" }),
    );
    assert_ne!(r["isError"], true, "{r}");
    assert_eq!(structured(&r)["restyled"], 2);
    let r = c.call(
        "align_items",
        json!({ "path": f, "page": 2, "item_ids": [stroke, text], "align": "top" }),
    );
    assert_ne!(r["isError"], true, "{r}");
    let r = c.call(
        "update_text",
        json!({ "path": f, "page": 2, "item_id": text, "text": "Leaf cell" }),
    );
    assert_ne!(r["isError"], true, "{r}");
    // a custom SVG stamp with behaviour, and a Smart stamp, placed in the file
    let svg =
        r#"<svg viewBox="0 0 100 100"><circle cx="50" cy="50" r="45" fill="currentColor"/></svg>"#;
    let st = structured(&c.call(
        "create_stamp",
        json!({ "path": f, "name": "Sun", "art": "svg", "svg": svg,
        "color": "#f5b301", "behavior": { "animation": "sparkle", "counter": true } }),
    ));
    let sid = st["id"].as_str().unwrap().to_string();
    let r = c.call(
        "add_stamp",
        json!({ "path": f, "page": 2, "stamp": sid, "x": 800, "y": 400 }),
    );
    assert_ne!(r["isError"], true, "{r}");
    let dice = r#"<!doctype html><title>Dice</title><body style="font:80px system-ui"><b id=d>1</b>
<script>document.body.onclick=()=>{d.textContent=1+Math.floor(Math.random()*6)}</script>"#;
    let r = c.call(
        "create_stamp",
        json!({ "path": f, "id": "dice", "name": "Dice", "art": "smart", "html": dice }),
    );
    assert_ne!(r["isError"], true, "{r}");
    let r = c.call(
        "add_stamp",
        json!({ "path": f, "page": 2, "stamp": "dice" }),
    );
    assert_ne!(r["isError"], true, "{r}");
    let stamps = structured(&c.call("list_stamps", json!({ "path": f })));
    assert_eq!(stamps["custom"].as_array().unwrap().len(), 2);
    // unsafe SVG is refused and nothing is saved
    let before = std::fs::read(dir.path().join(f)).unwrap();
    let r = c.call(
        "create_stamp",
        json!({ "path": f, "name": "Bad", "art": "svg", "svg": "<svg onload=\"x()\"/>" }),
    );
    assert_eq!(r["isError"], true);
    assert!(
        structured(&r)["error"]
            .as_str()
            .unwrap()
            .contains("svg refused")
    );
    assert_eq!(std::fs::read(dir.path().join(f)).unwrap(), before);
    // pages
    let r = structured(&c.call("duplicate_pages", json!({ "path": f, "pages": [2] })));
    assert_eq!(r["copies"], json!([3]));
    let r = c.call("move_pages", json!({ "path": f, "pages": [3], "to": 1 }));
    assert_ne!(r["isError"], true, "{r}");
    let pages = structured(&c.call("list_pages", json!({ "path": f })));
    assert_eq!(pages["page_count"], 3);
    assert_eq!(
        pages["pages"][0]["objects"], 1,
        "the copy (with the Smart stamp) is first: {pages}"
    );
    // board-only tools need the live board
    let r = c.call("set_mode", json!({ "mode": "teach" }));
    assert_eq!(r["isError"], true);
    assert!(
        structured(&r)["error"]
            .as_str()
            .unwrap()
            .contains("no board is paired")
    );
    // settings that are the teacher's alone never reach the board
    let r = c.call(
        "set_setting",
        json!({ "key": "claudeKey", "value": "sk-x" }),
    );
    assert_eq!(r["isError"], true);
    assert!(
        structured(&r)["error"]
            .as_str()
            .unwrap()
            .contains("teacher's alone")
    );
    // the PDF goes in the workspace only
    let r = c.call("export_pdf", json!({ "save_as": "../out.pdf" }));
    assert_eq!(r["isError"], true);

    // list + spec
    let lessons = structured(&c.call("list_lessons", json!({})));
    assert_eq!(lessons["lessons"][0]["title"], "How plants grow");
    let spec = c.call("read_object_spec", json!({}));
    assert!(
        spec["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("rupoklib://")
    );

    // Closing stdin ends the session cleanly.
    drop(c.stdin.take());
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = c.child.try_wait().unwrap() {
            assert!(status.success(), "server exited with {status}");
            break;
        }
        assert!(
            start.elapsed() < TIMEOUT,
            "server did not exit after stdin closed"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
