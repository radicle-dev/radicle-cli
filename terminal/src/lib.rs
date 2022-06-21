#![allow(clippy::collapsible_if)]

pub mod command;
#[cfg(feature = "ethereum")]
pub mod ethereum;
pub mod format;
pub mod io;
pub mod keys;
pub mod patch;
pub mod spinner;
pub mod sync;
pub mod table;
pub mod textbox;
pub mod tui;

use std::process;

use dialoguer::console::style;
use radicle_common::args::{Args, Error, Help};

pub use console::measure_text_width as text_width;
pub use dialoguer::Editor;
pub use io::*;
pub use spinner::{spinner, Spinner};
pub use table::Table;
pub use textbox::TextBox;

pub fn run_command<A, F>(help: Help, action: &str, run: F) -> !
where
    A: Args,
    F: FnOnce(A) -> anyhow::Result<()>,
{
    use crate::io as term;

    let options = match A::from_env() {
        Ok(opts) => opts,
        Err(err) => {
            match err.downcast_ref::<Error>() {
                Some(Error::Help) => {
                    term::help(help.name, help.version, help.description, help.usage);
                    process::exit(0);
                }
                Some(Error::Usage) => {
                    term::usage(help.name, help.usage);
                    process::exit(1);
                }
                _ => {}
            }
            eprintln!(
                "{} {} {} {}",
                style("==").red(),
                style("Error:").red(),
                style(format!("rad-{}:", help.name)).red(),
                style(err).red()
            );
            process::exit(1);
        }
    };

    match run(options) {
        Ok(()) => process::exit(0),
        Err(err) => {
            term::fail(&format!("{} failed", action), &err);
            process::exit(1);
        }
    }
}
