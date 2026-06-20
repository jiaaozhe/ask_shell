use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::shell::Shell;

const DETECT_TIMEOUT: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq)]
pub struct Environment {
    pub os: String,
    pub arch: String,
    pub kernel: Option<String>,
    pub shell_version: Option<String>,
}

impl Environment {
    pub fn detect(shell: &Shell) -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            kernel: capture_first_line("uname", &["-srm"]),
            shell_version: capture_first_line(
                shell.path.to_string_lossy().as_ref(),
                &["--version"],
            ),
        }
    }
}

fn capture_first_line(program: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();

    while child.try_wait().ok()?.is_none() {
        if start.elapsed() >= DETECT_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        thread::sleep(Duration::from_millis(10));
    }

    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detect_includes_static_os_and_arch() {
        let shell = Shell {
            path: PathBuf::from("/bin/sh"),
            name: "sh".to_string(),
        };

        let environment = Environment::detect(&shell);

        assert_eq!(environment.os, std::env::consts::OS);
        assert_eq!(environment.arch, std::env::consts::ARCH);
    }
}
