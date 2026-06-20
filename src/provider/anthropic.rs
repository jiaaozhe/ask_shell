use anyhow::{anyhow, Context, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    config::Config,
    provider::{ai_response_schema, parse_ai_response, AiProvider, AiResponse, Message, Role},
};

pub struct AnthropicProvider {
    client: Client,
    base_url: String,
    model: String,
    api_key: String,
    temperature: f64,
}

impl AnthropicProvider {
    pub fn new(config: Config) -> Result<Self> {
        let api_key = config.api_key()?;
        Ok(Self {
            client: Client::new(),
            base_url: config.base_url.trim_end_matches('/').to_string(),
            model: config.model,
            api_key,
            temperature: config.temperature.unwrap_or(0.2),
        })
    }
}

#[async_trait::async_trait]
impl AiProvider for AnthropicProvider {
    async fn ask(&self, messages: &[Message]) -> Result<AiResponse> {
        let body = match self.send_request(messages, true).await {
            Ok(body) => body,
            Err(error) if error.structured_output_might_be_unsupported => self
                .send_request(messages, false)
                .await
                .map_err(|error| error.error)?,
            Err(error) => return Err(error.error),
        };
        let content = body
            .content
            .into_iter()
            .map(|block| match block {
                ContentBlock::Text { text } => text,
            })
            .next()
            .ok_or_else(|| anyhow!("Anthropic API response did not include text content"))?;

        parse_ai_response(&content).context("failed to parse model JSON response")
    }
}

impl AnthropicProvider {
    async fn send_request(
        &self,
        messages: &[Message],
        use_structured_output: bool,
    ) -> std::result::Result<MessagesResponse, AnthropicResponseError> {
        let (system, conversation) = split_messages(messages);
        let request = MessagesRequest {
            model: &self.model,
            max_tokens: 1000,
            temperature: self.temperature,
            system: system.as_deref(),
            messages: conversation,
            output_config: use_structured_output.then(OutputConfig::json_schema),
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .map_err(|error| AnthropicResponseError {
                error: anyhow!(error).context("failed to call Anthropic API"),
                structured_output_might_be_unsupported: false,
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AnthropicResponseError {
                error: anyhow!("Anthropic API returned {status}: {body}"),
                structured_output_might_be_unsupported: use_structured_output
                    && status == StatusCode::BAD_REQUEST,
            });
        }

        let body: MessagesResponse =
            response
                .json()
                .await
                .map_err(|error| AnthropicResponseError {
                    error: anyhow!(error).context("failed to parse Anthropic API response"),
                    structured_output_might_be_unsupported: false,
                })?;

        Ok(body)
    }
}

struct AnthropicResponseError {
    error: anyhow::Error,
    structured_output_might_be_unsupported: bool,
}

fn split_messages(messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage<'_>>) {
    let mut system = Vec::new();
    let mut conversation = Vec::new();

    for message in messages {
        match message.role {
            Role::System => system.push(message.content.as_str()),
            Role::User => conversation.push(AnthropicMessage {
                role: "user",
                content: &message.content,
            }),
            Role::Assistant => conversation.push(AnthropicMessage {
                role: "assistant",
                content: &message.content,
            }),
        }
    }

    let system = if system.is_empty() {
        None
    } else {
        Some(system.join("\n\n"))
    };

    (system, conversation)
}

#[derive(Debug, Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    temperature: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<AnthropicMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_config: Option<OutputConfig>,
}

#[derive(Debug, Serialize)]
struct OutputConfig {
    format: OutputFormat,
}

impl OutputConfig {
    fn json_schema() -> Self {
        Self {
            format: OutputFormat {
                kind: "json_schema",
                schema: ai_response_schema(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct OutputFormat {
    #[serde(rename = "type")]
    kind: &'static str,
    schema: Value,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ContentBlock {
    Text { text: String },
}
