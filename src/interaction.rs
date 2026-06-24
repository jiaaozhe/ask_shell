use anyhow::Result;

use crate::{
    conversation,
    environment::Environment,
    execute, prompt,
    provider::{AiProvider, AiResponse},
    shell::Shell,
    status::Status,
    terminal::{self, CommandOutcome, InspectRequest, PostRunDecision},
};

#[derive(Debug, Clone, Copy)]
enum ModelCallReason {
    Initial,
    UserAnswer,
    InspectResult,
    Feedback,
    CommandResult,
}

impl ModelCallReason {
    fn status_message(self) -> &'static str {
        match self {
            Self::Initial => "Calling model",
            Self::UserAnswer => "Sending your answer to model",
            Self::InspectResult => "Sending inspect result to model",
            Self::Feedback => "Sending feedback to model",
            Self::CommandResult => "Sending command result to model",
        }
    }
}

pub async fn resolve_command(
    provider: &(dyn AiProvider + Send + Sync),
    shell: &Shell,
    environment: &Environment,
    request: String,
    print_only: bool,
) -> Result<Option<String>> {
    let mut messages = prompt::initial_messages(&request, shell, environment);
    let mut next_model_call = ModelCallReason::Initial;

    loop {
        let status = Status::start(format!("{}...", next_model_call.status_message()));
        let response = provider.ask(&messages).await?;
        drop(status);

        match response {
            AiResponse::Question { question } => {
                eprintln!();
                eprintln!("{question}");
                let answer = terminal::read_answer()?;
                conversation::push_question_exchange(&mut messages, question, answer);
                next_model_call = ModelCallReason::UserAnswer;
            }
            AiResponse::Inspect { command, reason } => {
                let inspect = InspectRequest::new(command, reason)?;

                if !terminal::review_inspect(&inspect)? {
                    return Ok(None);
                }

                let status = Status::start(format!("Running inspect: {}", inspect.command));
                let output = execute::capture(shell, &inspect.command).await?;
                drop(status);
                terminal::print_inspect_output(&output);
                conversation::push_inspect_exchange(&mut messages, &inspect, output);
                next_model_call = ModelCallReason::InspectResult;
            }
            AiResponse::Command { command, note } => {
                let command = command.trim().to_string();
                terminal::validate_command(&command)?;
                match terminal::review_command(command, note)? {
                    CommandOutcome::Run(command) => {
                        if print_only {
                            return Ok(Some(command));
                        }

                        let status = Status::start(format!("Running: {command}"));
                        let output = execute::run_captured(shell, &command).await?;
                        drop(status);

                        match terminal::review_command_result(&output)? {
                            PostRunDecision::Done => return Ok(None),
                            PostRunDecision::Continue(feedback) => {
                                conversation::push_command_result_exchange(
                                    &mut messages,
                                    &command,
                                    &output,
                                    &feedback,
                                );
                                next_model_call = ModelCallReason::CommandResult;
                            }
                        }
                    }
                    CommandOutcome::Feedback {
                        command,
                        note,
                        feedback,
                    } => {
                        conversation::push_command_feedback_exchange(
                            &mut messages,
                            command,
                            note,
                            feedback,
                        );
                        next_model_call = ModelCallReason::Feedback;
                    }
                    CommandOutcome::Cancel => return Ok(None),
                }
            }
        }
    }
}
