mod config;
mod config_command;
mod conversation;
mod environment;
mod execute;
mod interaction;
mod prompt;
mod provider;
mod shell;
mod status;
mod terminal;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "ask")]
#[command(about = "Generate and approve shell commands from natural language")]
struct Cli {
    /// Print the approved command to stdout instead of executing it.
    #[arg(long)]
    print_command: bool,

    #[command(subcommand)]
    command: Option<Command>,

    /// Natural language command request.
    #[arg(value_name = "PROMPT")]
    prompt: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Manage ask configuration.
    Config {
        #[command(subcommand)]
        command: config_command::ConfigCommand,
    },
    /// Print shell integration for command history support.
    ShellInit {
        #[arg(value_enum)]
        shell: ShellInitShell,
    },
}

#[derive(Debug, Clone, ValueEnum)]
enum ShellInitShell {
    Bash,
    Zsh,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(command) = cli.command {
        match command {
            Command::Config { command } => return config_command::run(command),
            Command::ShellInit { shell } => {
                print_shell_init(shell);
                return Ok(());
            }
        }
    }

    config_command::ensure_initialized()?;
    let config = config::Config::load()?;

    let request = if cli.prompt.is_empty() {
        terminal::read_initial_request()?
    } else {
        cli.prompt.join(" ")
    };

    let shell = shell::detect_shell()?;
    let environment = environment::Environment::detect(&shell);
    let provider = provider::build_provider(config)?;

    if let Some(command) = interaction::resolve_command(
        provider.as_ref(),
        &shell,
        &environment,
        request,
        cli.print_command,
    )
    .await?
    {
        println!("{command}");
    }

    Ok(())
}

fn print_shell_init(shell: ShellInitShell) {
    match shell {
        ShellInitShell::Bash => {
            println!(
                r#"ask() {{
  if [[ $# -eq 0 ]]; then
    command ask
    return
  fi

  local cmd
  cmd="$(command ask --print-command "$@")" || return
  [[ -n "$cmd" ]] || return
  history -s "$cmd"
  eval "$cmd"
}}"#
            );
        }
        ShellInitShell::Zsh => {
            println!(
                r#"ask() {{
  if [[ $# -eq 0 ]]; then
    command ask
    return
  fi

  local cmd
  cmd="$(command ask --print-command "$@")" || return
  [[ -n "$cmd" ]] || return
  print -s -- "$cmd"
  eval "$cmd"
}}"#
            );
        }
    }
}
