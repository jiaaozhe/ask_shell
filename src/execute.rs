use std::{io, process::Stdio, time::Duration};

use anyhow::{Context, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
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

/// Runs an approved command, streaming its output to the terminal live while
/// capturing it so a failed or unsatisfying result can be fed back to the model.
pub async fn run_captured(shell: &Shell, command: &str) -> Result<CommandOutput> {
    let mut child = Command::new(&shell.path)
        .arg("-lc")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to run command with {}", shell.path.display()))?;

    let stdout = child
        .stdout
        .take()
        .context("piped stdout was not attached")?;
    let stderr = child
        .stderr
        .take()
        .context("piped stderr was not attached")?;

    let (stdout, stderr) = tokio::join!(
        copy_and_capture(stdout, tokio::io::stdout()),
        copy_and_capture(stderr, tokio::io::stderr()),
    );

    let (stdout, stdout_truncated) = stdout.context("failed to capture stdout")?;
    let (stderr, stderr_truncated) = stderr.context("failed to capture stderr")?;

    let status = child
        .wait()
        .await
        .with_context(|| format!("failed to wait for command with {}", shell.path.display()))?;

    Ok(CommandOutput {
        exit_code: status.code(),
        success: status.success(),
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    })
}

/// Copies a stream to a writer in real time while accumulating up to
/// `CAPTURE_LIMIT_BYTES` for later inspection.
async fn copy_and_capture<R, W>(mut reader: R, mut writer: W) -> io::Result<(String, bool)>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = [0u8; 8 * 1024];
    let mut collected: Vec<u8> = Vec::new();
    let mut truncated = false;

    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }

        writer.write_all(&buf[..n]).await?;
        writer.flush().await?;

        if !truncated {
            let remaining = CAPTURE_LIMIT_BYTES.saturating_sub(collected.len());
            if remaining > 0 {
                collected.extend_from_slice(&buf[..n.min(remaining)]);
            }
            if collected.len() >= CAPTURE_LIMIT_BYTES {
                truncated = true;
            }
        }
    }

    Ok((String::from_utf8_lossy(&collected).to_string(), truncated))
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
