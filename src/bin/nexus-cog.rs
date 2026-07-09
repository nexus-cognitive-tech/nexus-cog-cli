//! `nexus-cog` — enterprise CLI entry point.
//!
//! See `cli.rs` for the clap definition; this file wires subcommands to
//! handlers and applies global flags. The MCP server mode is reached via
//! the `mcp` subcommand — handled asynchronously by [`run_mcp`].

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use nexus_cog_cli::cli::{Cli, Cmd, ShellChoice};
use nexus_cog_cli::commands::common;
use nexus_cog_cli::commands::{
    antifragile, backup, brain, causal, cognitive, config, decay, embedder, intent, intel,
    palace, patterns, provenance,
};
use nexus_cog_cli::completion;
use nexus_cog_cli::config::{CliConfig, Profile};
use nexus_cog_cli::ctx::{expand_tilde, Ctx};
use nexus_cog_cli::format::OutputFormat;
use nexus_cog_cli::mcp::{run as run_mcp, McpArgs};
use nexus_cog_cli::repl;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose.log_level_filter());

    // Load global config and resolve the profile.
    let cfg = CliConfig::load_default().unwrap_or_default();
    let profile: Option<&Profile> = cfg.resolve(cli.profile.as_deref());

    // Per-workspace DB by default: `<workspace>/.nexus-cog/palace.db`. The
    // previous global default (`~/.local/share/nexus-cog/palace.db`) leaked
    // state across unrelated agents and has been removed.
    let workspace: PathBuf = cli
        .workspace
        .clone()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let db: PathBuf = cli
        .db
        .clone()
        .or_else(|| profile.and_then(|p| p.db.clone()))
        .map(|p| expand_tilde(&p))
        .unwrap_or_else(|| {
            let dir = workspace.join(".nexus-cog");
            std::fs::create_dir_all(&dir).ok();
            dir.join("palace.db")
        });
    let format = cli.format;

    // Commands that don't need a palace open.
    match &cli.cmd {
        Cmd::Completions { shell } => {
            let shell = match shell {
                ShellChoice::Bash => clap_complete::Shell::Bash,
                ShellChoice::Zsh => clap_complete::Shell::Zsh,
                ShellChoice::Fish => clap_complete::Shell::Fish,
                ShellChoice::Powershell => clap_complete::Shell::PowerShell,
                ShellChoice::Elvish => clap_complete::Shell::Elvish,
            };
            let mut buf = std::io::stdout();
            completion::emit(shell, &mut buf)?;
            return Ok(());
        }
        Cmd::Config(cmd) => return run_config(cmd, format),
        Cmd::Embedder(cmd) => return run_embedder(cmd, format),
        Cmd::Doctor => return run_doctor(),
        // The MCP server is handled asynchronously below.
        Cmd::Mcp { db, workspace } => {
            let args = McpArgs {
                db: db.as_ref().map(|p| p.to_string_lossy().into_owned()),
                workspace: workspace.as_ref().map(|p| p.to_string_lossy().into_owned()),
            };
            // Drop any tracing init — rmcp owns the stdio pipe and tracing
            // chatter on stderr can confuse some MCP hosts.
            return run_mcp(args).await;
        }
        _ => {}
    }

    let mut ctx = Ctx::open(db).context("open cortex context")?;

    match cli.cmd {
        Cmd::Config(_) | Cmd::Embedder(_) | Cmd::Completions { .. } | Cmd::Doctor | Cmd::Mcp { .. } => {
            unreachable!()
        }
        Cmd::Palace(c) => run_palace(&mut ctx, c, format),
        Cmd::Brain(c) => run_brain(&ctx, c, format),
        Cmd::Cognitive(c) => run_cognitive(&mut ctx, c, format),
        Cmd::Causal(c) => run_causal(&mut ctx, c, format),
        Cmd::Patterns(c) => run_patterns(&ctx, c, format),
        Cmd::Provenance(c) => run_provenance(&mut ctx, c, format),
        Cmd::Intel(c) => run_intel(&mut ctx, c, format),
        Cmd::Intent(c) => run_intent(&mut ctx, c, format),
        Cmd::Antifragile(c) => run_antifragile(&ctx, c, format),
        Cmd::Backup(c) => run_backup(&ctx, c, format),
        Cmd::Decay => {
            let (half_life_days, min_importance, replay_per_cycle) = decay::default_config();
            let r = decay::apply(&ctx, half_life_days, min_importance, replay_per_cycle)?;
            common::print(&ctx, decay::report_to_value(&r))?;
            ctx.save()?;
            Ok(())
        }
        Cmd::Repl => repl::run(&mut ctx, format),
        Cmd::Mcp { .. } => unreachable!(),
    }
}

fn run_palace(ctx: &mut Ctx, c: nexus_cog_cli::cli::PalaceCmd, format: OutputFormat) -> Result<()> {
    use nexus_cog_cli::cli::PalaceCmd as P;
    let v = match c {
        P::Rooms => palace::rooms(ctx)?,
        P::Summary => palace::summary(ctx)?,
        P::AddRoom { name, r#type } => palace::add_room(ctx, &name, Some(&r#type))?,
        P::AddItem { room, key, value, confidence, tags } => {
            palace::add_item(ctx, &room, &key, &value, Some(confidence), tags)?
        }
        P::Recall { query, limit } => palace::recall(ctx, &query, limit, None, None, None)?,
        P::Connect { from, to, relation, strength } => {
            palace::connect(ctx, &from, &to, &relation, Some(strength))?
        }
    };
    common::print(ctx, render_with(v, format))?;
    Ok(())
}

fn run_brain(ctx: &Ctx, c: nexus_cog_cli::cli::BrainCmd, format: OutputFormat) -> Result<()> {
    use nexus_cog_cli::cli::BrainCmd as B;
    let v = match c {
        B::Verify { code } => brain::verify(&code)?,
        B::Risks { code, file } => brain::risks(&code, Some(&file))?,
        B::Search { query, code, path, limit } => {
            brain::search(&query, &[(path.clone(), code)], limit)?
        }
        B::Architecture { code, path } => {
            let corpus = vec![(path.clone(), code)];
            brain::architecture(&corpus)?
        }
        B::Graph { code, path } => {
            let corpus = vec![(path.clone(), code)];
            brain::graph(&corpus)?
        }
        B::Diff { old, new, file } => brain::diff(&file, &old, &new)?,
        B::Hypothesis { title, description, code_a, code_b } => {
            brain::hypothesis(&title, &description, &code_a, &code_b, None)?
        }
        B::File { path } => brain::analyze_file(&path)?,
    };
    common::print(ctx, render_with(v, format))?;
    Ok(())
}

fn run_cognitive(ctx: &mut Ctx, c: nexus_cog_cli::cli::CognitiveCmd, format: OutputFormat) -> Result<()> {
    use nexus_cog_cli::cli::CognitiveCmd as C;
    let v = match c {
        C::Think { task, context } => cognitive::think(ctx, &task, Some(&context), None)?,
        C::Mirror { subject, response } => cognitive::mirror(ctx, &subject, &response)?,
        C::ChainStart => cognitive::start_chain(ctx)?,
        C::ChainAdd { r#type, content, confidence } => {
            cognitive::add_thought(ctx, &r#type, &content, Some(confidence))?
        }
        C::Analyze { response } => cognitive::analyze_response(ctx, &response)?,
    };
    common::print(ctx, render_with(v, format))?;
    Ok(())
}

fn run_causal(ctx: &mut Ctx, c: nexus_cog_cli::cli::CausalCmd, format: OutputFormat) -> Result<()> {
    use nexus_cog_cli::cli::CausalCmd as C;
    let v = match c {
        C::AddNode { id, name, r#type, description } => {
            causal::add_node(ctx, &id, &name, Some(&r#type), Some(&description))?
        }
        C::AddEdge { from, to } => causal::add_edge(ctx, &from, &to, None, None)?,
        C::Forward { entity } => causal::forward(ctx, &entity)?,
        C::Backward { entity } => causal::backward(ctx, &entity)?,
        C::Counterfactual { entity } => causal::counterfactual(ctx, &entity)?,
        C::PreMortem { entity } => causal::pre_mortem(ctx, &entity)?,
        C::Blast { entity } => causal::blast(ctx, &entity)?,
        C::Dump => causal::dump(ctx)?,
    };
    common::print(ctx, render_with(v, format))?;
    Ok(())
}

fn run_patterns(ctx: &Ctx, c: nexus_cog_cli::cli::PatternsCmd, format: OutputFormat) -> Result<()> {
    use nexus_cog_cli::cli::PatternsCmd as P;
    let v = match c {
        P::List => patterns::list(ctx)?,
        P::Match { code } => patterns::match_code(ctx, &code)?,
        P::Suggest { task } => patterns::suggest(ctx, &task)?,
    };
    common::print(ctx, render_with(v, format))?;
    Ok(())
}

fn run_provenance(ctx: &mut Ctx, c: nexus_cog_cli::cli::ProvenanceCmd, format: OutputFormat) -> Result<()> {
    use nexus_cog_cli::cli::ProvenanceCmd as P;
    let v = match c {
        P::Record { artifact, origin, content, source, prompt } => {
            provenance::record(
                ctx,
                &artifact,
                &origin,
                &content,
                &source,
                &prompt,
                None,
                None,
                None,
            )?
        }
        P::Explain { id } => provenance::explain(ctx, &id, None)?,
        P::Search { query } => provenance::search(ctx, &query)?,
    };
    common::print(ctx, render_with(v, format))?;
    Ok(())
}

fn run_intel(ctx: &mut Ctx, c: nexus_cog_cli::cli::IntelCmd, format: OutputFormat) -> Result<()> {
    use nexus_cog_cli::cli::IntelCmd as I;
    let v = match c {
        I::Recall { query, limit, category, min_importance } => intel::recall(
            ctx,
            &query,
            limit,
            category.as_deref(),
            min_importance,
        )?,
        I::Store { key, value, category, importance } => {
            intel::store(ctx, &key, &value, Some(&category), Some(importance))?
        }
        I::Stats => intel::stats(ctx)?,
        I::LearnerStats => intel::learner_stats(ctx)?,
        I::Predict { task, tools } => intel::predict(ctx, &task, &tools)?,
        I::Record { task, success, quality, rounds, tools } => {
            intel::record_interaction(ctx, &task, Some(success), Some(quality), Some(rounds), tools)?
        }
        I::Suggest { task, complexity } => {
            intel::suggest_approach(ctx, &task, Some(&complexity))?
        }
    };
    common::print(ctx, render_with(v, format))?;
    Ok(())
}

fn run_intent(ctx: &mut Ctx, c: nexus_cog_cli::cli::IntentCmd, format: OutputFormat) -> Result<()> {
    use nexus_cog_cli::cli::IntentCmd as I;
    let v = match c {
        I::Declare { module, purpose } => intent::declare(ctx, &module, &purpose)?,
        I::Check { module, current_code, strict } => {
            intent::check(ctx, &module, &current_code, strict)?
        }
        I::Drift { module, observation, drift_score } => {
            intent::drift(ctx, &module, &observation, Some(drift_score))?
        }
        I::Index => intent::index(ctx)?,
    };
    common::print(ctx, render_with(v, format))?;
    Ok(())
}

fn run_antifragile(ctx: &Ctx, c: nexus_cog_cli::cli::AntifragileCmd, format: OutputFormat) -> Result<()> {
    use nexus_cog_cli::cli::AntifragileCmd as A;
    let v = match c {
        A::Adversarial { target } => {
            antifragile::adversarial(ctx, Some(&target), None, None, None, None)?
        }
        A::Edge { code, target } => antifragile::edge_cases(ctx, &code, &target)?,
    };
    common::print(ctx, render_with(v, format))?;
    Ok(())
}

fn run_backup(ctx: &Ctx, c: nexus_cog_cli::cli::BackupCmd, format: OutputFormat) -> Result<()> {
    use nexus_cog_cli::cli::BackupCmd as B;
    let v = match c {
        B::Json { out } => backup::export_json(ctx, &out)?,
        B::Sqlite { dst } => backup::backup_sqlite(ctx, &dst)?,
    };
    common::print(ctx, render_with(v, format))?;
    Ok(())
}

fn run_config(cmd: &nexus_cog_cli::cli::ConfigCmd, _format: OutputFormat) -> Result<()> {
    use nexus_cog_cli::cli::ConfigCmd as C;
    let v = match cmd {
        C::Show => config::show()?,
        C::Init => config::init()?,
        C::AddProfile { name, db } => {
            config::add_profile(name, db.as_deref())?
        }
    };
    print!("{}", serde_json::to_string_pretty(&v)?);
    println!();
    Ok(())
}

fn run_embedder(cmd: &nexus_cog_cli::cli::EmbedderCmd, _format: OutputFormat) -> Result<()> {
    use nexus_cog_cli::cli::EmbedderCmd as E;
    let v = match cmd {
        E::Info { kind } => embedder::info(*kind)?,
    };
    print!("{}", serde_json::to_string_pretty(&v)?);
    println!();
    Ok(())
}

fn run_doctor() -> Result<()> {
    println!("nexus-cog {}", env!("CARGO_PKG_VERSION"));
    println!("binary: {}", std::env::current_exe()?.display());
    let v = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "rust_version": "rustc 1.x",
        "config": CliConfig::default_path().ok().map(|p| p.display().to_string()),
    });
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

fn init_tracing(_level: log::LevelFilter) {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::builder()
        .with_default_directive(
            tracing_subscriber::filter::LevelFilter::from_level(tracing::Level::WARN).into(),
        )
        .from_env_lossy();
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn render_with(value: serde_json::Value, _format: OutputFormat) -> serde_json::Value {
    value
}
