use std::io::{self, IsTerminal, Write};

use anyhow::{bail, Result};
use dialoguer::{theme::ColorfulTheme, Select};
use rustyline::DefaultEditor;

use crate::execute::CommandOutput;

const MAX_REASON_CHARS: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandDecision {
    Run,
    Edit,
    GiveFeedback,
    Cancel,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandOutcome {
    Run(String),
    Feedback {
        command: String,
        note: String,
        feedback: String,
    },
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectDecision {
    Run,
    Cancel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InspectRequest {
    pub command: String,
    pub reason: String,
}

impl InspectRequest {
    pub fn new(command: String, reason: String) -> Result<Self> {
        let command = command.trim().to_string();
        validate_command(&command)?;

        Ok(Self {
            command,
            reason: trim_reason(&reason),
        })
    }
}

pub fn read_initial_request() -> Result<String> {
    eprintln!("What command do you need?");
    let input = read_prompted_line("> ")?;
    if input.trim().is_empty() {
        bail!("empty request");
    }
    Ok(input)
}

pub fn read_answer() -> Result<String> {
    let answer = read_prompted_line("> ")?;
    if answer.trim().is_empty() {
        bail!("empty answer");
    }
    Ok(answer)
}

pub fn review_inspect(inspect: &InspectRequest) -> Result<bool> {
    eprintln!();
    eprintln!("Inspect:");
    eprintln!("{}", inspect.command);
    if !inspect.reason.trim().is_empty() {
        eprintln!();
        eprintln!("Reason:");
        eprintln!("{}", inspect.reason.trim());
    }
    eprintln!();

    Ok(matches!(choose_inspect_decision()?, InspectDecision::Run))
}

pub fn print_inspect_output(output: &CommandOutput) {
    eprintln!();
    eprintln!("Inspect output:");
    if !output.stdout.is_empty() {
        eprint!("{}", output.stdout);
        if !output.stdout.ends_with('\n') {
            eprintln!();
        }
    }
    if !output.stderr.is_empty() {
        eprint!("{}", output.stderr);
        if !output.stderr.ends_with('\n') {
            eprintln!();
        }
    }
    if output.stdout.is_empty() && output.stderr.is_empty() {
        eprintln!("(no output)");
    }
    if output.stdout_truncated || output.stderr_truncated {
        eprintln!("(output truncated)");
    }
}

pub fn review_command(mut command: String, note: String) -> Result<CommandOutcome> {
    eprintln!();
    eprintln!("Command:");
    eprintln!("{command}");
    if !note.trim().is_empty() {
        eprintln!();
        eprintln!("Note:");
        eprintln!("{}", note.trim());
    }
    eprintln!();
    match choose_command_decision()? {
        CommandDecision::Run => Ok(CommandOutcome::Run(command)),
        CommandDecision::Edit => {
            command = read_edited_command(&command)?.trim().to_string();
            validate_command(&command)?;
            Ok(CommandOutcome::Run(command))
        }
        CommandDecision::GiveFeedback => Ok(CommandOutcome::Feedback {
            command,
            note,
            feedback: read_feedback()?,
        }),
        CommandDecision::Cancel => Ok(CommandOutcome::Cancel),
    }
}

pub fn validate_command(command: &str) -> Result<()> {
    if command.trim().is_empty() {
        bail!("model returned an empty command");
    }
    if command.contains('\n') || command.contains('\r') {
        bail!("model returned a multi-line command; only one-line commands are supported");
    }
    Ok(())
}

fn choose_inspect_decision() -> Result<InspectDecision> {
    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        let items = ["Run", "Cancel"];
        let selection = select_action(&items, 0)?;

        return Ok(match selection {
            Some(0) => InspectDecision::Run,
            _ => InspectDecision::Cancel,
        });
    }

    loop {
        eprint!("Run inspect? [y/N]: ");
        io::stderr().flush()?;
        match read_line()?.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(InspectDecision::Run),
            "" | "n" | "no" => return Ok(InspectDecision::Cancel),
            _ => {
                eprintln!("Please answer y or n.");
            }
        }
    }
}

fn choose_command_decision() -> Result<CommandDecision> {
    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        let items = ["Run", "Edit", "Give Feedback", "Cancel"];
        let selection = select_action(&items, 0)?;

        return Ok(match selection {
            Some(0) => CommandDecision::Run,
            Some(1) => CommandDecision::Edit,
            Some(2) => CommandDecision::GiveFeedback,
            _ => CommandDecision::Cancel,
        });
    }

    loop {
        eprintln!();
        eprint!("Run? [y/N/e/f]: ");
        io::stderr().flush()?;

        match read_line()?.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(CommandDecision::Run),
            "e" | "edit" => return Ok(CommandDecision::Edit),
            "f" | "feedback" => return Ok(CommandDecision::GiveFeedback),
            "" | "n" | "no" => return Ok(CommandDecision::Cancel),
            _ => {
                eprintln!("Please answer y, n, e, or f.");
            }
        }
    }
}

fn select_action(items: &[&str], default: usize) -> Result<Option<usize>> {
    Ok(Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Action")
        .items(items)
        .default(default)
        .interact_opt()?)
}

fn read_feedback() -> Result<String> {
    eprintln!();
    eprintln!("What should be changed?");
    let feedback = read_prompted_line("> ")?;
    if feedback.trim().is_empty() {
        bail!("empty feedback");
    }
    Ok(feedback)
}

fn trim_reason(reason: &str) -> String {
    let reason = reason.trim().replace(['\r', '\n'], " ");
    let mut chars = reason.chars();
    let trimmed: String = chars.by_ref().take(MAX_REASON_CHARS).collect();
    if chars.next().is_some() {
        format!("{trimmed}...")
    } else {
        trimmed
    }
}

fn read_line() -> Result<String> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim_end_matches(['\r', '\n']).to_string())
}

fn read_prompted_line(prompt: &str) -> Result<String> {
    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        let mut editor = DefaultEditor::new()?;
        Ok(editor.readline(prompt)?)
    } else {
        eprint!("{prompt}");
        io::stderr().flush()?;
        read_line()
    }
}

fn read_edited_command(command: &str) -> Result<String> {
    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        let mut editor = DefaultEditor::new()?;
        Ok(editor.readline_with_initial("Edit command: ", (command, ""))?)
    } else {
        eprint!("Edit command: ");
        io::stderr().flush()?;
        read_line()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspect_request_trims_command_and_reason() {
        let request = InspectRequest::new(
            "  git status --short  ".to_string(),
            "  确认\n仓库\r状态  ".to_string(),
        )
        .unwrap();

        assert_eq!(
            request,
            InspectRequest {
                command: "git status --short".to_string(),
                reason: "确认 仓库 状态".to_string(),
            }
        );
    }

    #[test]
    fn inspect_request_rejects_multiline_command() {
        let error = InspectRequest::new("pwd\nls".to_string(), "check".to_string()).unwrap_err();

        assert!(error
            .to_string()
            .contains("multi-line command; only one-line commands are supported"));
    }

    #[test]
    fn trim_reason_truncates_long_reasons() {
        let reason = "a".repeat(MAX_REASON_CHARS + 1);

        assert_eq!(
            trim_reason(&reason),
            format!("{}...", "a".repeat(MAX_REASON_CHARS))
        );
    }
}
