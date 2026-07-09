//! Interactive REPL.
//!
//! `nexus-cog repl` opens a shell where every line is treated as a
//! hippocampal recall query against the active cortex. History is
//! persisted to `~/.local/share/nexus-cog/repl_history` (best-effort).

use anyhow::Result;
use rustyline::history::FileHistory;
use rustyline::Editor;

use crate::ctx::Ctx;
use crate::format::{Fmt, OutputFormat};

pub fn run(ctx: &mut Ctx, fmt: OutputFormat) -> Result<()> {
    let mut rl: Editor<(), FileHistory> = Editor::new()?;

    let history_path = crate::config::CliConfig::default_path()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("repl_history")));
    if let Some(hp) = &history_path {
        let _ = rl.load_history(hp);
    }

    let f = Fmt::new(fmt);
    println!("nexus-cog REPL — type a query to recall from the cortex. Ctrl-D to exit.");
    loop {
        let readline = rl.readline("nexus-cog> ");
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line == ":quit" || line == ":q" || line == "exit" {
                    break;
                }
                if line == ":help" || line == ":h" {
                    println!("commands: :help :quit | otherwise: recall query");
                    continue;
                }
                let _ = rl.add_history_entry(line);
                recall(ctx, &f, line);
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("(Ctrl-C — use Ctrl-D or :quit to exit)");
            }
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("error: {e}");
                break;
            }
        }
    }
    if let Some(hp) = history_path {
        let _ = rl.save_history(&hp);
    }
    Ok(())
}

fn recall(ctx: &Ctx, fmt: &Fmt, query: &str) {
    use crate::commands::intel;
    let hits = ctx.cortex.hippocampus_recall(
        &intel::encode_text_to_sdr_pub(query),
        10,
        None,
    );
    let value = serde_json::to_value(&hits).unwrap_or(serde_json::Value::Null);
    let mut out = std::io::stdout();
    let _ = fmt.render(&value, &mut out);
}
