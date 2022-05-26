use dialoguer::console::style;
use indicatif::{ProgressBar, ProgressFinish, ProgressStyle};

use crate as term;

pub struct Spinner {
    progress: ProgressBar,
    message: String,
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if !self.progress.is_finished() {
            self.finish()
        }
    }
}

impl Spinner {
    pub fn finish(&self) {
        self.progress.finish_and_clear();
        term::success!("{}", &self.message);
    }

    pub fn done(self) {
        self.progress.finish_and_clear();
        term::info!("{}", &self.message);
    }

    pub fn failed(self) {
        self.progress.finish_and_clear();
        term::eprintln(style("!!").red().reverse(), &self.message);
    }

    pub fn error(self, err: anyhow::Error) {
        self.progress.finish_and_clear();
        term::eprintln(style("!!").red().reverse(), &self.message);
        term::eprintln("  ", style(err).red());
    }

    pub fn clear(self) {
        self.progress.finish_and_clear();
    }

    pub fn message(&mut self, msg: impl Into<String>) {
        let msg = msg.into();

        self.progress.set_message(msg.clone());
        self.message = msg;
    }
}

pub fn spinner(message: &str) -> Spinner {
    let message = message.to_owned();
    let style = ProgressStyle::default_spinner()
        .tick_strings(&[
            &style("\\ ").yellow().to_string(),
            &style("| ").yellow().to_string(),
            &style("/ ").yellow().to_string(),
            &style("| ").yellow().to_string(),
        ])
        .template("{spinner} {msg}")
        .on_finish(ProgressFinish::AndClear);

    let progress = ProgressBar::new(!0);
    progress.set_style(style);
    progress.enable_steady_tick(99);
    progress.set_message(message.clone());

    Spinner { message, progress }
}
