//! Shell completion script generation.

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{generate, Shell};

use crate::cli::Cli;

pub fn emit(shell: Shell, buf: &mut dyn std::io::Write) -> Result<()> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    generate(shell, &mut cmd, name, buf);
    Ok(())
}
