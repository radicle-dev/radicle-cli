use std::ffi::OsString;
use std::process;
use std::str::FromStr;

use anyhow::anyhow;

use crate::components as term;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// If this error is returned from argument parsing, help is displayed.
    #[error("help invoked")]
    Help,
    /// If this error is returned from argument parsing, usage is displayed.
    #[error("usage invoked")]
    Usage,
    /// An error with a hint.
    #[error("{err}")]
    WithHint {
        err: anyhow::Error,
        hint: &'static str,
    },
}

pub struct Help {
    pub name: &'static str,
    pub description: &'static str,
    pub version: &'static str,
    pub usage: &'static str,
}

pub trait Args: Sized {
    fn from_env() -> anyhow::Result<Self> {
        let args = std::env::args_os().into_iter().skip(1).collect();

        match Self::from_args(args) {
            Ok((opts, unparsed)) => {
                self::finish(unparsed)?;

                Ok(opts)
            }
            Err(err) => Err(err),
        }
    }

    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)>;
}

pub fn run_command<A, F>(help: Help, action: &str, run: F) -> !
where
    A: Args,
    F: FnOnce(A) -> anyhow::Result<()>,
{
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
            term::failure(help.name, &err);
            process::exit(1);
        }
    };

    match run(options) {
        Ok(()) => process::exit(0),
        Err(err) => {
            term::format::error(&format!("{} failed", action), &err);
            process::exit(1);
        }
    }
}

pub fn parse_value<T: FromStr>(flag: &str, value: OsString) -> anyhow::Result<T>
where
    <T as FromStr>::Err: std::error::Error,
{
    value
        .into_string()
        .map_err(|_| anyhow!("the value specified for '--{}' is not valid unicode", flag))?
        .parse()
        .map_err(|e| anyhow!("invalid value specified for '--{}' ({})", flag, e))
}

pub fn format(arg: lexopt::Arg) -> OsString {
    match arg {
        lexopt::Arg::Long(flag) => format!("--{}", flag).into(),
        lexopt::Arg::Short(flag) => format!("-{}", flag).into(),
        lexopt::Arg::Value(val) => val,
    }
}

pub fn finish(unparsed: Vec<OsString>) -> anyhow::Result<()> {
    if let Some(arg) = unparsed.first() {
        return Err(anyhow::anyhow!(
            "unexpected argument `{}`",
            arg.to_string_lossy()
        ));
    }
    Ok(())
}
