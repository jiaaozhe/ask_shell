use std::{process::Stdio, time::Duration};

use anyhow::{Context, Result};
use tokio::process::Command;
use tokio::time::timeout;

use crate::shell::Shell;

const INSPECT_TIMEOUT: Duration = Duration::from_secs(10);
const CAPTURE_LIMIT_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandOutput {
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

/// Runs an approved command and captures its output so a failed or
/// unsatisfying result can be fed back to the model.
pub async fn run_captured(shell: &Shell, command: &str) -> Result<CommandOutput> {
    let output = Command::new(&shell.path)
        .arg("-lc")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await
        .with_context(|| format!("failed to run command with {}", shell.path.display()))?;
    Ok(split_output(output))
}

/// Runs a read-only inspect command and captures its output, bounded by
/// `INSPECT_TIMEOUT`.
pub async fn capture(shell: &Shell, command: &str) -> Result<CommandOutput> {
    let mut child = Command::new(&shell.path);
    child.arg("-lc").arg(command).kill_on_drop(true);

    let output = timeout(INSPECT_TIMEOUT, child.output())
        .await
        .with_context(|| {
            format!(
                "inspect command timed out after {} seconds",
                INSPECT_TIMEOUT.as_secs()
            )
        })?
        .with_context(|| {
            format!(
                "failed to run inspect command with {}",
                shell.path.display()
            )
        })?;

    Ok(split_output(output))
}

fn split_output(output: std::process::Output) -> CommandOutput {
    let (stdout, stdout_truncated) = truncate_output(&output.stdout);
    let (stderr, stderr_truncated) = truncate_output(&output.stderr);

    CommandOutput {
        exit_code: output.status.code(),
        success: output.status.success(),
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    }
}

fn truncate_output(bytes: &[u8]) -> (String, bool) {
    let truncated = bytes.len() > CAPTURE_LIMIT_BYTES;
    let visible = if truncated {
        &bytes[..CAPTURE_LIMIT_BYTES]
    } else {
        bytes
    };

    (String::from_utf8_lossy(visible).to_string(), truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_shell() -> Shell {
        Shell {
            path: PathBuf::from("/bin/sh"),
            name: "sh".to_string(),
        }
    }

    #[test]
    fn truncates_large_output() {
        let bytes = vec![b'a'; CAPTURE_LIMIT_BYTES + 1];
        let (output, truncated) = truncate_output(&bytes);

        assert!(truncated);
        assert_eq!(output.len(), CAPTURE_LIMIT_BYTES);
    }

    #[test]
    fn keeps_small_output() {
        let (output, truncated) = truncate_output(b"ok");

        assert!(!truncated);
        assert_eq!(output, "ok");
    }

    #[tokio::test]
    async fn run_captured_collects_stdout_and_success_status() {
        let output = run_captured(&test_shell(), "printf ok").await.unwrap();

        assert_eq!(output.exit_code, Some(0));
        assert!(output.success);
        assert_eq!(output.stdout, "ok");
        assert_eq!(output.stderr, "");
        assert!(!output.stdout_truncated);
        assert!(!output.stderr_truncated);
    }

    #[tokio::test]
    async fn run_captured_collects_stderr_and_failed_status() {
        let output = run_captured(&test_shell(), "printf problem >&2; exit 7")
            .await
            .unwrap();

        assert_eq!(output.exit_code, Some(7));
        assert!(!output.success);
        assert_eq!(output.stdout, "");
        assert_eq!(output.stderr, "problem");
    }

    #[tokio::test]
    async fn capture_collects_stdout_and_success_status() {
        let output = capture(&test_shell(), "printf ok").await.unwrap();

        assert_eq!(output.exit_code, Some(0));
        assert!(output.success);
        assert_eq!(output.stdout, "ok");
        assert_eq!(output.stderr, "");
        assert!(!output.stdout_truncated);
        assert!(!output.stderr_truncated);
    }

    #[tokio::test]
    async fn capture_collects_stderr_and_failed_status() {
        let output = capture(&test_shell(), "printf problem >&2; exit 7")
            .await
            .unwrap();

        assert_eq!(output.exit_code, Some(7));
        assert!(!output.success);
        assert_eq!(output.stdout, "");
        assert_eq!(output.stderr, "problem");
    }
}
