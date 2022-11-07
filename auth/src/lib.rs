#![allow(clippy::or_fun_call)]
use std::ffi::OsString;

use anyhow::Context as _;

use zeroize::Zeroizing;

use radicle::Profile;

use radicle_common::args::{Args, Error, Help};
use radicle_common::{git, keys};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "auth",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad auth [--init | --active] [<options>...] [<peer-id>]

    A passphrase may be given via the environment variable `RAD_PASSPHRASE` or
    via the standard input stream if `--stdin` is used. Using one of these
    methods disables the passphrase prompt.

    If `--init` is used, a name may be given via the `--name` option. Using
    this disables the input prompt.

Options

    --init                  Initialize a new identity
    --active                Authenticate with the currently active profile
    --stdin                 Read passphrase from stdin (default: false)
    --name <name>           Use given name (default: none)
    --help                  Print help
"#,
};

#[derive(Debug)]
pub struct Options {
    pub active: bool,
    pub stdin: bool,
    pub name: Option<String>,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut init = false;
        let mut active = false;
        let mut stdin = false;
        let mut name = None;
        let mut parser = lexopt::Parser::from_args(args);

        while let Some(arg) = parser.next()? {
            match arg {
                Long("init") => {
                    init = true;
                }
                Long("active") => {
                    active = true;
                }
                Long("stdin") => {
                    stdin = true;
                }
                Long("name") if init && name.is_none() => {
                    let val = parser
                        .value()?
                        .to_str()
                        .ok_or(anyhow::anyhow!("invalid name specified with `--name`"))?
                        .to_owned();

                    name = Some(val);
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                active,
                stdin,
                name,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = Profile::load();

    if profile.is_err() {
        init(options)
    } else {
        authenticate(options, ctx)
    }
}

pub fn init(options: Options) -> anyhow::Result<()> {
    term::headline("Initializing your 🌱 profile and identity");

    if git::check_version().is_err() {
        term::warning(&format!(
            "Your git version is unsupported, please upgrade to {} or later",
            git::VERSION_REQUIRED,
        ));
        term::blank();
    }

    let passphrase = term::read_passphrase(options.stdin, true)?;
    let secret = passphrase.unsecure();

    let spinner = term::spinner("Creating your 🌱 Ed25519 keypair...");
    let profile = Profile::init(secret)?;
    spinner.finish();

    term::success!(
        "Profile {} created.",
        term::format::highlight(&profile.id().to_string())
    );

    term::blank();
    term::info!(
        "Your radicle Node ID is {}. This identifies your device.",
        term::format::highlight(&profile.id().to_string())
    );

    term::blank();
    term::tip!(
        "To create a radicle project, run {} from a git repository.",
        term::format::secondary("`rad init`")
    );

    Ok(())
}

pub fn authenticate(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = match ctx.profile() {
        Ok(profile) => profile,
        Err(_) => {
            anyhow::bail!(
                "Active identity could not be loaded.\n\
                To create a new identity, run `rad auth --init`."
            )
        }
    };

    if !options.active {
        term::info!(
            "Your active identity is {}",
            term::display::Identity::new().styled()
        );
    }

    term::headline(&format!(
        "🌱 Authenticating as {}",
        term::display::Identity::new().styled()
    ));

    let profile = &profile;
    if !keys::is_ready(profile)? {
        term::warning("Adding your radicle key to ssh-agent...");

        // TODO: We should show the spinner on the passphrase prompt,
        // otherwise it seems like the passphrase is valid even if it isn't.
        let passphrase = term::read_passphrase(options.stdin, false)?;
        let secret = Zeroizing::new(passphrase.unsecure().to_string());

        let spinner = term::spinner("Unlocking...");
        keys::add(profile, secret).context("invalid passphrase supplied")?;
        spinner.finish();

        term::success!("Radicle key added to ssh-agent");
    } else {
        term::success!("Signing key already in ssh-agent");
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use assay::assay;

    use super::*;

    use radicle_common::test;

    fn create_auth_options(name: &str) -> Options {
        Options {
            active: false,
            stdin: false,
            name: Some(name.to_owned()),
        }
    }

    #[assay(
        setup = test::setup::lnk_home()?,
        teardown = test::teardown::profiles()?,
    )]
    fn can_be_initialized() {
        let options = create_auth_options("user");

        init(options).unwrap();

        assert_eq!(profile::count().unwrap(), 1);
        assert_eq!(profile::name(None).unwrap(), "user");
    }

    #[assay(
        setup = test::setup::lnk_home()?,
    )]
    fn name_cannot_contain_whitespace() {
        let options = create_auth_options("user A");

        assert!(init(options).is_err());
    }
}
