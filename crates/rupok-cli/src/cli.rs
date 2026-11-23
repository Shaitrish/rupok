//! The `rupok` command line (clap). Each subcommand is a thin wrapper over
//! [`crate::ops`].

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, Subcommand};
use rupok_format::{Diagnostic, Severity};

use crate::ops::{self, OpError};

#[derive(Debug, Parser)]
#[command(
    name = "rupok",
    version,
    about = "Create, inspect and validate rupok lessons (.rpk), and serve them to AI agents over MCP"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new lesson file with blank 1920×1080 slide pages.
    New {
        /// Lesson file to create (must end in .rpk and not exist yet).
        file: PathBuf,
        /// Lesson title.
        #[arg(long)]
        title: String,
        /// Number of pages.
        #[arg(long, default_value_t = 1)]
        pages: u32,
    },
    /// Show a lesson's title, pages, Smart Objects and assets.
    Info {
        file: PathBuf,
        /// Print machine-readable JSON.
        #[arg(long, conflicts_with = "md")]
        json: bool,
        /// Print the lesson as Markdown (the package's content.md): text,
        /// recognised handwriting, shapes, images and Smart Objects per page.
        #[arg(long)]
        md: bool,
    },
    /// Export a lesson as readable text for people, scripts and AI agents.
    Export {
        file: PathBuf,
        /// Markdown outline (the package's content.md). The default.
        #[arg(long, conflicts_with = "content_json")]
        md: bool,
        /// Structured outline (the package's content.json).
        #[arg(long = "content-json")]
        content_json: bool,
        /// Write to this file instead of standard output.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Validate a Smart Object bundle directory, a single .html file, or every
    /// object in a .rpk lesson. Exits 1 if there are errors.
    Validate {
        target: PathBuf,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
        /// Build steps to declare when validating a bare .html file.
        #[arg(long)]
        steps: Option<u32>,
    },
    /// Add a Smart Object (bundle directory or .html file) to a lesson.
    /// Refuses objects that fail validation.
    AddObject {
        /// Lesson to modify.
        lesson: PathBuf,
        /// Bundle directory (with manifest.json) or a self-contained .html file.
        source: PathBuf,
        /// 1-based page number (default: last page).
        #[arg(long)]
        page: Option<usize>,
        /// Left edge in slide pixels (default: centred in the 1920×1080 frame).
        #[arg(long, allow_negative_numbers = true)]
        x: Option<f64>,
        /// Top edge in slide pixels (default: centred in the 1920×1080 frame).
        #[arg(long, allow_negative_numbers = true)]
        y: Option<f64>,
        /// Width in slide pixels (default: the manifest width; 1280 for .html).
        #[arg(long)]
        w: Option<f64>,
        /// Height in slide pixels (default: the manifest height; 720 for .html).
        #[arg(long)]
        h: Option<f64>,
        /// Object title (default: manifest title, or the .html <title> / file name).
        #[arg(long)]
        title: Option<String>,
        /// Number of build steps (default: manifest value, or 0 for .html).
        #[arg(long)]
        steps: Option<u32>,
    },
    /// Run the MCP server on stdio so an AI agent can create and edit lessons.
    Mcp {
        /// Directory the agent may read and write lessons in (default: $RUPOK_LIBRARY,
        /// else the Rupok library folder ~/Documents/Rupok if it exists, else the
        /// current dir).
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Open the live board channel so a paired board can be edited during
        /// class (the default; accepted for explicit configs).
        #[arg(long, conflicts_with = "no_live")]
        live: bool,
        /// Do not open the live board channel: file tools only.
        #[arg(long)]
        no_live: bool,
        /// Port of the live channel on 127.0.0.1 (default: the first free of
        /// 47373, 47374, 47375, 47376, which the board probes).
        #[arg(long)]
        live_port: Option<u16>,
        /// Extra board origin allowed to connect (a self-hosted board), e.g.
        /// https://board.school.example. Repeatable.
        #[arg(long = "live-origin", value_name = "ORIGIN")]
        live_origins: Vec<String>,
    },
    /// Chat with your own AI agent inside Rupok: relay the board's chat panel
    /// to Claude Code, Codex, OpenCode or Gemini CLI (over ACP), with the
    /// Rupok tools attached so it builds on the board live.
    Agent {
        /// Lessons folder (default: as for `rupok mcp`).
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Agent to use first: claude, codex, opencode, gemini (default: the
        /// first installed). The teacher can switch in the chat panel.
        #[arg(long)]
        agent: Option<String>,
        /// A custom ACP agent command, e.g. "my-agent --acp".
        #[arg(long, value_name = "COMMAND")]
        agent_command: Option<String>,
        /// Name shown for --agent-command.
        #[arg(long, default_value = "Custom agent")]
        agent_name: String,
        /// Port of the live channel on 127.0.0.1 (default: first free of 47373–47376).
        #[arg(long)]
        live_port: Option<u16>,
        /// Extra board origin allowed to connect. Repeatable.
        #[arg(long = "live-origin", value_name = "ORIGIN")]
        live_origins: Vec<String>,
        /// Open the board in the browser, already paired.
        #[arg(long)]
        open: bool,
        /// Board address for --open.
        #[arg(long, default_value = "https://rupok.app/board/")]
        board_url: String,
    },
}

/// Entry point used by `main`.
pub fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}

pub fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    match cli.command {
        Command::New { file, title, pages } => {
            if !crate::workspace::is_lesson_path(&file) {
                anyhow::bail!("{}: lesson files must end in .rpk", file.display());
            }
            let created = ops::create_lesson(&file, &title, pages)?;
            println!(
                "Created {} \"{}\" with {} page(s)",
                file.display(),
                created.title,
                created.page_ids.len()
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Info { file, md: true, .. } => {
            print!("{}", ops::readable(&file, false)?);
            Ok(ExitCode::SUCCESS)
        }
        Command::Export {
            file,
            md: _,
            content_json,
            output,
        } => {
            let text = ops::readable(&file, content_json)?;
            match output {
                Some(out) => std::fs::write(&out, text)
                    .with_context(|| format!("writing {}", out.display()))?,
                None => print!("{text}"),
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Info { file, json, .. } => {
            let info = ops::info(&file)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&info)?);
            } else {
                print_info(&file.display().to_string(), &info);
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Validate {
            target,
            json,
            steps,
        } => {
            let v = ops::validate_path(&target, steps)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&v)?);
            } else {
                print_validation(&v);
            }
            Ok(if v.valid {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Command::AddObject {
            lesson,
            source,
            page,
            x,
            y,
            w,
            h,
            title,
            steps,
        } => {
            if !crate::workspace::is_lesson_path(&lesson) {
                anyhow::bail!("{}: lesson files must end in .rpk", lesson.display());
            }
            let opts = ops::SourceOptions { title, w, h, steps };
            let bundle = ops::load_source(&source, &opts)
                .with_context(|| format!("loading {}", source.display()))?;
            match ops::add_object(&lesson, bundle, ops::Placement { page, x, y, w, h }) {
                Ok(added) => {
                    print_diagnostics(&added.warnings);
                    let g = added.geometry;
                    println!(
                        "Added object {} (bundle {}) to page {} at {:.0},{:.0} {:.0}×{:.0}",
                        added.item_id, added.bundle_id, added.page, g.x, g.y, g.w, g.h
                    );
                    Ok(ExitCode::SUCCESS)
                }
                Err(OpError::Diagnostics(d)) => {
                    print_diagnostics(&d);
                    eprintln!(
                        "refusing to add {}: {} error(s)",
                        source.display(),
                        ops::error_count(&d)
                    );
                    Ok(ExitCode::FAILURE)
                }
                Err(e) => Err(e.into()),
            }
        }
        Command::Agent {
            workspace,
            agent,
            agent_command,
            agent_name,
            live_port,
            live_origins,
            open,
            board_url,
        } => {
            let dir = pick_workspace(
                workspace,
                std::env::var_os("RUPOK_LIBRARY")
                    .filter(|v| !v.is_empty())
                    .map(PathBuf::from),
                library_dir().filter(|d| d.is_dir()),
                std::env::current_dir()?,
            );
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(async move {
                let started = crate::chat::start(crate::chat::AgentOptions {
                    workspace: dir.clone(),
                    default_agent: agent,
                    custom: agent_command.map(|c| (agent_name, c)),
                    ports: live_port.map(|p| vec![p]),
                    extra_origins: live_origins,
                    code: None,
                })
                .await
                .with_context(|| "starting the Rupok agent relay")?;
                let code = crate::live::spaced(&started.code);
                println!(
                    "Rupok agent relay is running (lessons folder: {}).",
                    dir.display()
                );
                println!();
                println!("  In Rupok, click the AI button and enter this code:  {code}");
                println!();
                println!("Leave this window open while you teach. Ctrl+C stops it.");
                if open {
                    let url = format!("{board_url}#rupok-agent={}:{}", started.port, started.code);
                    open_browser(&url);
                }
                tokio::signal::ctrl_c().await.ok();
                anyhow::Ok(())
            })?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Mcp {
            workspace,
            live: _,
            no_live,
            live_port,
            live_origins,
        } => {
            let dir = pick_workspace(
                workspace,
                std::env::var_os("RUPOK_LIBRARY")
                    .filter(|v| !v.is_empty())
                    .map(PathBuf::from),
                library_dir().filter(|d| d.is_dir()),
                std::env::current_dir()?,
            );
            let ws = crate::workspace::Workspace::new(&dir)
                .with_context(|| format!("workspace {}", dir.display()))?;
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(async move {
                if no_live {
                    return crate::mcp::serve_stdio(ws).await;
                }
                let mut cfg = crate::live::LiveConfig {
                    extra_origins: live_origins,
                    ..Default::default()
                };
                if let Some(p) = live_port {
                    cfg.ports = vec![p];
                }
                let hub = crate::live::LiveHub::new(cfg);
                // stdout is the MCP channel: log to stderr only.
                match hub.start().await {
                    Ok(port) => eprintln!("rupok: live board channel on ws://127.0.0.1:{port}"),
                    Err(e) => {
                        eprintln!("rupok: live board channel unavailable ({e}); file tools only")
                    }
                }
                crate::mcp::serve_stdio_with(ws, hub).await
            })?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// The Rupok library folder the desktop app uses by default
/// (`Documents/Rupok`, see docs/library.md), so agents and the app see the
/// same lessons.
pub fn library_dir() -> Option<PathBuf> {
    dirs::document_dir().map(|d| d.join("Rupok"))
}

/// Open `url` in the default browser (best effort).
fn open_browser(url: &str) {
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    if cmd.arg(url).spawn().is_err() {
        println!("Open {url} in your browser.");
    }
}

/// The MCP workspace: `--workspace`, else `$RUPOK_LIBRARY`, else the library
/// folder when it exists, else the current directory.
fn pick_workspace(
    explicit: Option<PathBuf>,
    env: Option<PathBuf>,
    library: Option<PathBuf>,
    cwd: PathBuf,
) -> PathBuf {
    explicit.or(env).or(library).unwrap_or(cwd)
}

fn print_diagnostics(d: &[Diagnostic]) {
    for d in d {
        let sev = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        eprintln!("{sev}[{}]: {}", d.code, d.message);
    }
}

fn print_validation(v: &ops::PathValidation) {
    if v.kind == "lesson" && v.objects.is_empty() {
        println!("{}: no Smart Objects", v.target);
    }
    for o in &v.objects {
        let name = match &o.id {
            Some(id) => format!("{} ({id})", v.target),
            None => v.target.clone(),
        };
        let r = &o.report;
        for d in &r.diagnostics {
            let sev = match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            println!("{sev}[{}]: {}", d.code, d.message);
        }
        let status = if r.valid { "ok" } else { "FAILED" };
        println!(
            "{name}: {status} — \"{}\": {} error(s), {} warning(s)",
            o.title, r.errors, r.warnings
        );
    }
}

fn human_bytes(n: usize) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

fn print_info(file: &str, info: &ops::LessonInfo) {
    println!("{}", info.title);
    println!("  file:      {file}");
    if let Some(g) = &info.generator {
        println!("  generator: {g}");
    }
    println!("  pages:     {}", info.page_count);
    for p in &info.pages {
        let title = if p.title.is_empty() {
            "(untitled)".to_string()
        } else {
            format!("\"{}\"", p.title)
        };
        let frame = match p.frame {
            Some(f) => format!("{:.0}×{:.0}", f.w, f.h),
            None => "infinite".into(),
        };
        let c = &p.items;
        let kinds: Vec<String> = [
            ("stroke", c.stroke),
            ("text", c.text),
            ("image", c.image),
            ("object", c.object),
        ]
        .iter()
        .filter(|(_, n)| *n > 0)
        .map(|(k, n)| format!("{n} {k}"))
        .collect();
        let kinds = if kinds.is_empty() {
            String::new()
        } else {
            format!(" ({})", kinds.join(", "))
        };
        println!(
            "    {:>3}  {:<24} {:<10} {:<6} {} item(s){kinds}",
            p.page, title, frame, p.pattern, p.item_count
        );
    }
    println!("  objects:   {}", info.objects.len());
    for o in &info.objects {
        let status = if o.valid {
            "valid".to_string()
        } else {
            format!("INVALID ({} error(s))", o.errors)
        };
        println!(
            "    {}  \"{}\"  {:.0}×{:.0}  {} step(s)  {}  {}  placed {}×",
            o.id,
            o.title,
            o.width,
            o.height,
            o.steps,
            human_bytes(o.bytes),
            status,
            o.placements
        );
    }
    println!(
        "  assets:    {} ({})",
        info.asset_count,
        human_bytes(info.asset_bytes)
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_defaults_to_the_library_folder() {
        use super::pick_workspace;
        use std::path::PathBuf;
        let p = |s: &str| Some(PathBuf::from(s));
        let cwd = PathBuf::from("/cwd");
        assert_eq!(
            pick_workspace(p("/w"), p("/env"), p("/lib"), cwd.clone()),
            PathBuf::from("/w")
        );
        assert_eq!(
            pick_workspace(None, p("/env"), p("/lib"), cwd.clone()),
            PathBuf::from("/env")
        );
        assert_eq!(
            pick_workspace(None, None, p("/lib"), cwd.clone()),
            PathBuf::from("/lib")
        );
        assert_eq!(pick_workspace(None, None, None, cwd.clone()), cwd);
        assert!(super::library_dir().is_none_or(|d| d.ends_with("Rupok")));
    }

    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn cli_commands_end_to_end() {
        let dir = tempfile::tempdir().unwrap();
        let lesson = dir.path().join("l.rpk");
        let run_args = |args: &[&str]| run(Cli::try_parse_from(args).unwrap());
        let l = lesson.to_str().unwrap();

        assert_eq!(
            run_args(&["rupok", "new", l, "--title", "Plants", "--pages", "2"]).unwrap(),
            ExitCode::SUCCESS
        );
        assert!(
            run_args(&["rupok", "new", l, "--title", "Again"]).is_err(),
            "must not overwrite"
        );

        let good = dir.path().join("good.html");
        std::fs::write(
            &good,
            "<title>Seed growth</title><h1 style='font-size:48px'>Seeds</h1>",
        )
        .unwrap();
        let bad = dir.path().join("bad.html");
        std::fs::write(
            &bad,
            r#"<script src="https://cdn.example.com/x.js"></script>"#,
        )
        .unwrap();

        assert_eq!(
            run_args(&["rupok", "validate", good.to_str().unwrap()]).unwrap(),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_args(&["rupok", "validate", bad.to_str().unwrap(), "--json"]).unwrap(),
            ExitCode::FAILURE
        );

        assert_eq!(
            run_args(&[
                "rupok",
                "add-object",
                l,
                good.to_str().unwrap(),
                "--page",
                "1",
                "--x",
                "-10"
            ])
            .unwrap(),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_args(&["rupok", "add-object", l, bad.to_str().unwrap()]).unwrap(),
            ExitCode::FAILURE
        );

        // A bundle directory.
        let bdir = dir.path().join("bundle");
        std::fs::create_dir(&bdir).unwrap();
        let b = rupok_format::ObjectBundle::from_single_html(
            "<p style='font-size:40px'>Hi</p>",
            "Dir object",
            800.0,
            600.0,
            0,
        );
        std::fs::write(
            bdir.join("manifest.json"),
            serde_json::to_vec(&b.manifest).unwrap(),
        )
        .unwrap();
        std::fs::write(bdir.join("index.html"), b.entry_html().unwrap()).unwrap();
        assert_eq!(
            run_args(&["rupok", "validate", bdir.to_str().unwrap()]).unwrap(),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_args(&["rupok", "add-object", l, bdir.to_str().unwrap()]).unwrap(),
            ExitCode::SUCCESS
        );

        let info = ops::info(&lesson).unwrap();
        assert_eq!(info.objects.len(), 2);
        assert_eq!(info.pages[0].items.object, 1);
        assert_eq!(
            info.pages[1].items.object, 1,
            "default page is the last one"
        );
        let dir_obj = info
            .objects
            .iter()
            .find(|o| o.title == "Dir object")
            .unwrap();
        assert_eq!((dir_obj.width, dir_obj.height), (800.0, 600.0));
        let o = ops::outline(&lesson).unwrap();
        let g = o.pages[1].items[0].geometry;
        assert_eq!(
            (g.x, g.y, g.w, g.h),
            (560.0, 240.0, 800.0, 600.0),
            "centred at manifest size"
        );
        assert_eq!(o.pages[0].items[0].geometry.x, -10.0);

        assert_eq!(
            run_args(&["rupok", "validate", l]).unwrap(),
            ExitCode::SUCCESS
        );
        assert_eq!(run_args(&["rupok", "info", l]).unwrap(), ExitCode::SUCCESS);
        assert_eq!(
            run_args(&["rupok", "info", l, "--json"]).unwrap(),
            ExitCode::SUCCESS
        );

        // A directory without a manifest is reported as a validation error, not a crash.
        let empty = dir.path().join("empty");
        std::fs::create_dir(&empty).unwrap();
        assert_eq!(
            run_args(&["rupok", "validate", empty.to_str().unwrap()]).unwrap(),
            ExitCode::FAILURE
        );
    }
}
