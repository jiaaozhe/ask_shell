use std::{io::IsTerminal, time::Duration};

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

pub struct Status {
    spinner: Option<ProgressBar>,
}

impl Status {
    pub fn start(message: impl Into<String>) -> Self {
        let message = message.into();
        if std::io::stderr().is_terminal() {
            let spinner = ProgressBar::new_spinner();
            spinner.set_draw_target(ProgressDrawTarget::stderr());
            spinner.set_style(
                ProgressStyle::with_template("{spinner} {msg}")
                    .unwrap_or_else(|_| ProgressStyle::default_spinner()),
            );
            spinner.set_message(message);
            spinner.enable_steady_tick(Duration::from_millis(80));
            Self {
                spinner: Some(spinner),
            }
        } else {
            eprintln!("{message}");
            Self { spinner: None }
        }
    }
}

impl Drop for Status {
    fn drop(&mut self) {
        if let Some(spinner) = &self.spinner {
            spinner.finish_and_clear();
        }
    }
}
