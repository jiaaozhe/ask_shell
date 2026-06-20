use anyhow::{anyhow, Context, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    config::Config,
    provider::{ai_response_schema, parse_ai_response, AiProvider, AiResponse, Message, Role},
};

pub struct OpenAiProvider {
    client: Client,
    base_url: String,
    model: String,
    api_key: String,
    temperature: f64,
}

impl OpenAiProvider {
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
impl AiProvider for OpenAiProvider {
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
            .choices
            .first()
            .map(|choice| choice.message.content.as_str())
            .ok_or_else(|| anyhow!("OpenAI-compatible API response did not include a message"))?;

        parse_ai_response(content).context("failed to parse model JSON response")
    }
}

impl OpenAiProvider {
    async fn send_request(
        &self,
        messages: &[Message],
        use_structured_output: bool,
    ) -> std::result::Result<ChatCompletionResponse, OpenAiResponseError> {
        let request = ChatCompletionRequest {
            model: &self.model,
            messages: messages.iter().map(OpenAiMessage::from).collect(),
            temperature: self.temperature,
            response_format: use_structured_output.then(ResponseFormat::json_schema),
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|error| OpenAiResponseError {
                error: anyhow!(error).context("failed to call OpenAI-compatible API"),
                structured_output_might_be_unsupported: false,
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(OpenAiResponseError {
                error: anyhow!("OpenAI-compatible API returned {status}: {body}"),
                structured_output_might_be_unsupported: use_structured_output
                    && status == StatusCode::BAD_REQUEST,
            });
        }

        let body: ChatCompletionResponse =
            response.json().await.map_err(|error| OpenAiResponseError {
                error: anyhow!(error).context("failed to parse OpenAI-compatible API response"),
                structured_output_might_be_unsupported: false,
            })?;

        Ok(body)
    }
}

struct OpenAiResponseError {
    error: anyhow::Error,
    structured_output_might_be_unsupported: bool,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<OpenAiMessage<'a>>,
    temperature: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: &'static str,
    json_schema: JsonSchemaFormat,
}

impl ResponseFormat {
    fn json_schema() -> Self {
        Self {
            kind: "json_schema",
            json_schema: JsonSchemaFormat {
                name: "ask_response",
                strict: true,
                schema: ai_response_schema(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct JsonSchemaFormat {
    name: &'static str,
    strict: bool,
    schema: Value,
}

#[derive(Debug, Serialize)]
struct OpenAiMessage<'a> {
    role: &'a str,
    content: &'a str,
}

impl<'a> From<&'a Message> for OpenAiMessage<'a> {
    fn from(message: &'a Message) -> Self {
        let role = match message.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        };

        Self {
            role,
            content: &message.content,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    content: String,
}
