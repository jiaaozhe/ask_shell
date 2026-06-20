use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Shell {
    pub path: PathBuf,
    pub name: String,
}

pub fn detect_shell() -> Result<Shell> {
    let shell = env::var("SHELL").context("SHELL is not set; only bash and zsh are supported")?;
    parse_shell(&shell)
}

fn parse_shell(shell: &str) -> Result<Shell> {
    let path = PathBuf::from(shell);
    let name = shell_name(&path)?;
    if name != "bash" && name != "zsh" {
        bail!(
            "unsupported shell '{}'; only bash and zsh are supported",
            name
        );
    }

    Ok(Shell { path, name })
}

fn shell_name(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .context("could not determine shell name from SHELL")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_bash_and_zsh() {
        assert_eq!(parse_shell("/bin/bash").unwrap().name, "bash");
        assert_eq!(parse_shell("/bin/zsh").unwrap().name, "zsh");
    }

    #[test]
    fn rejects_other_shells() {
        assert!(parse_shell("/bin/fish").is_err());
    }
}
