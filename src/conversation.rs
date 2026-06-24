use crate::{execute::CommandOutput, provider::Message, terminal::InspectRequest};

pub fn push_question_exchange(messages: &mut Vec<Message>, question: String, answer: String) {
    messages.push(Message::assistant(
        serde_json::json!({
            "type": "question",
            "question": question,
        })
        .to_string(),
    ));
    messages.push(Message::user(answer));
}

pub fn push_inspect_exchange(
    messages: &mut Vec<Message>,
    inspect: &InspectRequest,
    output: CommandOutput,
) {
    messages.push(Message::assistant(
        serde_json::json!({
            "type": "inspect",
            "command": &inspect.command,
            "reason": &inspect.reason,
        })
        .to_string(),
    ));
    messages.push(Message::user(
        serde_json::json!({
            "type": "inspect_result",
            "command": &inspect.command,
            "exit_code": output.exit_code,
            "success": output.success,
            "stdout": output.stdout,
            "stderr": output.stderr,
            "stdout_truncated": output.stdout_truncated,
            "stderr_truncated": output.stderr_truncated,
        })
        .to_string(),
    ));
}

pub fn push_command_feedback_exchange(
    messages: &mut Vec<Message>,
    command: String,
    note: String,
    feedback: String,
) {
    messages.push(Message::assistant(
        serde_json::json!({
            "type": "command",
            "command": command,
            "note": note,
        })
        .to_string(),
    ));
    messages.push(Message::user(feedback));
}

pub fn push_command_result_exchange(
    messages: &mut Vec<Message>,
    command: &str,
    output: &CommandOutput,
    feedback: &str,
) {
    messages.push(Message::assistant(
        serde_json::json!({
            "type": "command",
            "command": command,
        })
        .to_string(),
    ));
    messages.push(Message::user(
        serde_json::json!({
            "type": "command_result",
            "command": command,
            "success": output.success,
            "exit_code": output.exit_code,
            "stdout": output.stdout,
            "stderr": output.stderr,
            "stdout_truncated": output.stdout_truncated,
            "stderr_truncated": output.stderr_truncated,
            "feedback": feedback,
        })
        .to_string(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Role;
    use serde_json::Value;

    #[test]
    fn push_inspect_exchange_records_request_and_result() {
        let inspect = InspectRequest {
            command: "printf ok".to_string(),
            reason: "确认输出".to_string(),
        };
        let output = CommandOutput {
            exit_code: Some(0),
            success: true,
            stdout: "ok".to_string(),
            stderr: String::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        };
        let mut messages = Vec::new();

        push_inspect_exchange(&mut messages, &inspect, output);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::Assistant);
        assert_eq!(messages[1].role, Role::User);

        let request: Value = serde_json::from_str(&messages[0].content).unwrap();
        assert_eq!(request["type"], "inspect");
        assert_eq!(request["command"], "printf ok");
        assert_eq!(request["reason"], "确认输出");

        let result: Value = serde_json::from_str(&messages[1].content).unwrap();
        assert_eq!(result["type"], "inspect_result");
        assert_eq!(result["command"], "printf ok");
        assert_eq!(result["exit_code"], 0);
        assert_eq!(result["success"], true);
        assert_eq!(result["stdout"], "ok");
        assert_eq!(result["stdout_truncated"], false);
    }

    #[test]
    fn push_command_result_exchange_records_command_and_result() {
        let output = CommandOutput {
            exit_code: Some(2),
            success: false,
            stdout: String::new(),
            stderr: "du: invalid option".to_string(),
            stdout_truncated: false,
            stderr_truncated: false,
        };
        let mut messages = Vec::new();

        push_command_result_exchange(&mut messages, "du -ah .", &output, "use macOS flags");

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::Assistant);
        assert_eq!(messages[1].role, Role::User);

        let command_msg: Value = serde_json::from_str(&messages[0].content).unwrap();
        assert_eq!(command_msg["type"], "command");
        assert_eq!(command_msg["command"], "du -ah .");

        let result_msg: Value = serde_json::from_str(&messages[1].content).unwrap();
        assert_eq!(result_msg["type"], "command_result");
        assert_eq!(result_msg["command"], "du -ah .");
        assert_eq!(result_msg["success"], false);
        assert_eq!(result_msg["exit_code"], 2);
        assert_eq!(result_msg["stderr"], "du: invalid option");
        assert_eq!(result_msg["feedback"], "use macOS flags");
    }
}
