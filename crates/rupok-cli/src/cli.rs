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
        #[arg(long)]
        json: bool,
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
        /// Directory the agent may read and write lessons in (default: current dir).
        #[arg(long)]
        workspace: Option<PathBuf>,
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
        Command::Info { file, json } => {
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
        Command::Mcp { workspace } => {
            let dir = match workspace {
                Some(d) => d,
                None => std::env::current_dir()?,
            };
            let ws = crate::workspace::Workspace::new(&dir)
                .with_context(|| format!("workspace {}", dir.display()))?;
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(crate::mcp::serve_stdio(ws))?;
            Ok(ExitCode::SUCCESS)
        }
    }
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
