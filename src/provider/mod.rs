mod anthropic;
mod openai;

use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::{Config, ProviderKind};

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: String) -> Self {
        Self {
            role: Role::System,
            content,
        }
    }

    pub fn user(content: String) -> Self {
        Self {
            role: Role::User,
            content,
        }
    }

    pub fn assistant(content: String) -> Self {
        Self {
            role: Role::Assistant,
            content,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AiResponse {
    Question { question: String },
    Inspect { command: String, reason: String },
    Command { command: String, note: String },
}

#[derive(Debug, Deserialize)]
struct RawAiResponse {
    #[serde(rename = "type")]
    kind: Option<String>,
    question: Option<String>,
    command: Option<String>,
    reason: Option<String>,
    note: Option<String>,
}

#[async_trait]
pub trait AiProvider {
    async fn ask(&self, messages: &[Message]) -> Result<AiResponse>;
}

pub fn build_provider(config: Config) -> Result<Box<dyn AiProvider + Send + Sync>> {
    match config.provider {
        ProviderKind::Openai => Ok(Box::new(openai::OpenAiProvider::new(config)?)),
        ProviderKind::Anthropic => Ok(Box::new(anthropic::AnthropicProvider::new(config)?)),
    }
}

fn parse_ai_response(content: &str) -> Result<AiResponse> {
    let trimmed = content.trim();
    let without_fence = trimmed
        .strip_prefix("```json")
        .and_then(|value| value.strip_suffix("```"))
        .or_else(|| {
            trimmed
                .strip_prefix("```")
                .and_then(|value| value.strip_suffix("```"))
        })
        .unwrap_or(trimmed)
        .trim();

    if let Ok(response) = serde_json::from_str(without_fence) {
        return Ok(response);
    }

    let raw: RawAiResponse = serde_json::from_str(without_fence)?;
    raw.into_ai_response()
}

pub(crate) fn ai_response_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": {
                "type": "string",
                "enum": ["question", "inspect", "command"],
                "description": "The response kind."
            },
            "question": {
                "type": "string",
                "description": "Follow-up question for the user. Empty unless type is question."
            },
            "command": {
                "type": "string",
                "description": "Single-line shell command. Empty unless type is inspect or command."
            },
            "reason": {
                "type": "string",
                "description": "Brief reason for an inspect command. Empty unless type is inspect."
            },
            "note": {
                "type": "string",
                "description": "Brief warning or caveat for the final command. Empty unless needed."
            }
        },
        "required": ["type", "question", "command", "reason", "note"],
        "additionalProperties": false
    })
}

impl RawAiResponse {
    fn into_ai_response(self) -> Result<AiResponse> {
        let kind = self.kind.as_deref().map(str::trim);
        match kind {
            Some("question") => Ok(AiResponse::Question {
                question: self.question.unwrap_or_default(),
            }),
            Some("inspect") => Ok(AiResponse::Inspect {
                command: self.command.unwrap_or_default(),
                reason: self.reason.unwrap_or_default(),
            }),
            Some("command") => Ok(AiResponse::Command {
                command: self.command.unwrap_or_default(),
                note: self.note.unwrap_or_default(),
            }),
            Some(other) => bail!("unsupported response type '{other}'"),
            None => self.infer_ai_response(),
        }
    }

    fn infer_ai_response(self) -> Result<AiResponse> {
        let question = self.question.unwrap_or_default();
        let command = self.command.unwrap_or_default();
        let reason = self.reason.unwrap_or_default();
        let note = self.note.unwrap_or_default();

        if !question.trim().is_empty() && command.trim().is_empty() {
            return Ok(AiResponse::Question { question });
        }
        if !command.trim().is_empty() && !reason.trim().is_empty() {
            return Ok(AiResponse::Inspect { command, reason });
        }
        if !command.trim().is_empty() {
            return Ok(AiResponse::Command { command, note });
        }

        bail!("model response did not include a recognizable type, question, or command")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_command_response() {
        let response = parse_ai_response(
            r#"{"type":"command","command":"du -ah . | sort -rh | head -n 10","note":"Reads file sizes."}"#,
        )
        .unwrap();

        assert_eq!(
            response,
            AiResponse::Command {
                command: "du -ah . | sort -rh | head -n 10".to_string(),
                note: "Reads file sizes.".to_string(),
            }
        );
    }

    #[test]
    fn parses_question_response_from_json_fence() {
        let response =
            parse_ai_response("```json\n{\"type\":\"question\",\"question\":\"Which dir?\"}\n```")
                .unwrap();

        assert_eq!(
            response,
            AiResponse::Question {
                question: "Which dir?".to_string(),
            }
        );
    }

    #[test]
    fn parses_inspect_response() {
        let response = parse_ai_response(
            r#"{"type":"inspect","command":"git status --short","reason":"确认仓库状态"}"#,
        )
        .unwrap();

        assert_eq!(
            response,
            AiResponse::Inspect {
                command: "git status --short".to_string(),
                reason: "确认仓库状态".to_string(),
            }
        );
    }

    #[test]
    fn parses_structured_command_response_with_all_fields() {
        let response = parse_ai_response(
            r#"{"type":"command","question":"","command":"wc -l src/*.rs","reason":"","note":""}"#,
        )
        .unwrap();

        assert_eq!(
            response,
            AiResponse::Command {
                command: "wc -l src/*.rs".to_string(),
                note: String::new(),
            }
        );
    }

    #[test]
    fn infers_command_response_without_type() {
        let response =
            parse_ai_response(r#"{"command":"find . -name '*.rs' | wc -l","note":""}"#).unwrap();

        assert_eq!(
            response,
            AiResponse::Command {
                command: "find . -name '*.rs' | wc -l".to_string(),
                note: String::new(),
            }
        );
    }

    #[test]
    fn infers_question_response_without_type() {
        let response = parse_ai_response(r#"{"question":"Which directory?"}"#).unwrap();

        assert_eq!(
            response,
            AiResponse::Question {
                question: "Which directory?".to_string(),
            }
        );
    }
}
