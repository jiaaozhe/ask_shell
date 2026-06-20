use std::time::Duration;

use anyhow::{bail, Context, Result};
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

pub async fn run(shell: &Shell, command: &str) -> Result<()> {
    let status = Command::new(&shell.path)
        .arg("-lc")
        .arg(command)
        .status()
        .await
        .with_context(|| format!("failed to run command with {}", shell.path.display()))?;

    if !status.success() {
        match status.code() {
            Some(code) => bail!("command exited with status {code}"),
            None => bail!("command terminated by signal"),
        }
    }

    Ok(())
}

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

    let (stdout, stdout_truncated) = truncate_output(&output.stdout);
    let (stderr, stderr_truncated) = truncate_output(&output.stderr);

    Ok(CommandOutput {
        exit_code: output.status.code(),
        success: output.status.success(),
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    })
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
