use crate::{environment::Environment, provider::Message, shell::Shell};

pub fn initial_messages(
    user_request: &str,
    shell: &Shell,
    environment: &Environment,
) -> Vec<Message> {
    vec![
        Message::system(system_prompt(shell, environment)),
        Message::user(user_request.to_string()),
    ]
}

pub fn system_prompt(shell: &Shell, environment: &Environment) -> String {
    let kernel = environment.kernel.as_deref().unwrap_or("unknown");
    let shell_version = environment.shell_version.as_deref().unwrap_or("unknown");

    format!(
        r#"You are a shell command assistant.

Environment:
- OS: {os}
- Architecture: {arch}
- Kernel: {kernel}
- Shell: {shell_name} ({shell_path})
- Shell version: {shell_version}

The user describes a command they want. Return exactly one JSON object and no Markdown.

The response object must always include these string fields:
- type: one of question, inspect, or command
- question
- command
- reason
- note

Use empty strings for fields that do not apply.

If the request lacks information needed to generate a safe, correct command, ask a concise follow-up question:
{{"type":"question","question":"...","command":"","reason":"","note":""}}

If command output is required before choosing the final command, request a read-only inspect command:
{{"type":"inspect","question":"","command":"...","reason":"...","note":""}}

If enough information is available, return a single-line final shell command and a note:
{{"type":"command","question":"","command":"...","reason":"","note":"..."}}

Rules:
- Commands must be one line.
- Do not wrap responses in Markdown or code fences.
- Do not omit the type field.
- Match the user's language for question and note.
- Prefer conservative commands.
- Use inspect only when the output is necessary to choose the final command.
- Prefer read-only inspect commands such as pwd, ls, find, git status, cat, sed, grep, rg, test, or command -v.
- The inspect reason must be brief: at most 40 Chinese characters or 12 English words.
- The inspect reason should only say why the information is needed.
- You may receive inspect results from the user as JSON with type "inspect_result".
- Use an empty note string unless a warning is necessary.
- Only include a note when the command modifies or deletes files, installs packages, changes permissions, uses sudo, performs network requests, affects system state, may be slow, or has a non-obvious caveat.
- When note is needed, keep it brief: one short sentence, no explanation of obvious command behavior.
- Do not execute anything yourself.
- Generate commands compatible with this OS and shell.
- Avoid GNU-only flags on macOS unless the relevant GNU tool is known to be installed.
- Avoid macOS/BSD-only flags on Linux.
- If compatibility depends on an installed tool, request an inspect command such as command -v or a version command.
"#,
        os = environment.os,
        arch = environment.arch,
        kernel = kernel,
        shell_name = shell.name,
        shell_path = shell.path.display(),
        shell_version = shell_version,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_shell() -> Shell {
        Shell {
            path: PathBuf::from("/bin/zsh"),
            name: "zsh".to_string(),
        }
    }

    fn test_environment() -> Environment {
        Environment {
            os: "macos".to_string(),
            arch: "aarch64".to_string(),
            kernel: Some("Darwin 24.0.0 arm64".to_string()),
            shell_version: Some("zsh 5.9".to_string()),
        }
    }

    #[test]
    fn initial_messages_include_system_prompt_and_user_request() {
        let messages = initial_messages("list files", &test_shell(), &test_environment());

        assert_eq!(messages.len(), 2);
        assert!(messages[0].content.contains("shell command assistant"));
        assert_eq!(messages[1].content, "list files");
    }

    #[test]
    fn system_prompt_documents_inspect_protocol_and_short_reason() {
        let prompt = system_prompt(&test_shell(), &test_environment());

        assert!(prompt.contains(r#""type":"inspect""#));
        assert!(prompt.contains("inspect_result"));
        assert!(prompt.contains("at most 40 Chinese characters or 12 English words"));
    }

    #[test]
    fn system_prompt_includes_environment_context() {
        let prompt = system_prompt(&test_shell(), &test_environment());

        assert!(prompt.contains("- OS: macos"));
        assert!(prompt.contains("- Architecture: aarch64"));
        assert!(prompt.contains("- Kernel: Darwin 24.0.0 arm64"));
        assert!(prompt.contains("- Shell: zsh (/bin/zsh)"));
        assert!(prompt.contains("- Shell version: zsh 5.9"));
        assert!(prompt.contains("Generate commands compatible with this OS and shell"));
    }
}
