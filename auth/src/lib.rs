use std::ffi::OsString;

use anyhow::Context as _;

use librad::crypto::keystore::pinentry::SecUtf8;

use rad_common::{git, keys, person, profile};
use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

pub const HELP: Help = Help {
    name: "auth",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad auth [--init | --active] [<options>...]

    If `--init` is used, name and password may be given via the `--name`
    and `--password` option. Using these disables the respective input prompt.

Options

    --init                  Initialize a new identity
    --active                Authenticate with the currently active profile
    --name <name>           Use given name (default: none)
    --password <password>   Use given password (default: none)
    --help                  Print help
"#,
};

#[derive(Debug)]
pub struct Options {
    pub init: bool,
    pub active: bool,
    pub name: Option<String>,
    pub password: Option<String>,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut init = false;
        let mut active = false;
        let mut name = None;
        let mut password = None;
        let mut parser = lexopt::Parser::from_args(args);

        while let Some(arg) = parser.next()? {
            match arg {
                Long("init") => {
                    init = true;
                }
                Long("active") => {
                    active = true;
                }
                Long("name") if init && name.is_none() => {
                    let val = parser
                        .value()?
                        .to_str()
                        .ok_or(anyhow::anyhow!("invalid name specified with `--name`"))?
                        .to_owned();

                    name = Some(val);
                }
                Long("password") if init && password.is_none() => {
                    let val = parser
                        .value()?
                        .to_str()
                        .ok_or(anyhow::anyhow!(
                            "invalid password specified with `--password`"
                        ))?
                        .to_owned();

                    term::warning(
                        "Passing a plain-text password is considered insecure. \
                        Please only use for testing purposes.",
                    );

                    password = Some(val);
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                init,
                active,
                name,
                password,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let profiles = match profile::list() {
        Ok(profiles) => profiles,
        _ => vec![],
    };

    if options.init || profiles.is_empty() {
        init(options)
    } else {
        authenticate(&profiles, options)
    }
}

pub fn init(options: Options) -> anyhow::Result<()> {
    let sock = keys::ssh_auth_sock();

    term::headline("Initializing your 🌱 profile and identity");

    if git::check_version().is_err() {
        term::warning(&format!(
            "Warning: Your git version is unsupported, please upgrade to {} or later",
            git::VERSION_REQUIRED,
        ));
        term::blank();
    }

    let name = options
        .name
        .unwrap_or_else(|| term::text_input("Name", None).unwrap());
    let pass = term::pwhash(
        options
            .password
            .map_or_else(term::secret_input_with_confirmation, |password| {
                SecUtf8::from(password)
            }),
    );

    let mut spinner = term::spinner("Creating your 🌱 Ed25519 keypair...");
    let (profile, peer_id) = lnk_profile::create(None, pass.clone())?;

    git::configure_signing(profile.paths().git_dir(), &peer_id)?;

    spinner.finish();
    spinner = term::spinner("Adding to ssh-agent...");

    let profile_id = keys::add(&profile, pass, sock.clone())?;
    let (signer, storage) = keys::storage(&profile, sock)?;

    spinner.finish();

    let person = person::create(&profile, &name, signer, &storage)?;
    person::set_local(&storage, &person);

    term::success!(
        "Profile {} created.",
        term::format::highlight(&profile_id.to_string())
    );

    check_ignored()?;

    term::blank();
    term::info!(
        "Your radicle Peer ID is {}. This identifies your device.",
        term::format::highlight(&peer_id.to_string())
    );
    term::info!(
        "Your personal 🌱 URN is {}. This identifies you across devices.",
        term::format::highlight(&person.urn().to_string())
    );

    term::blank();
    term::tip!(
        "To create a radicle project, run {} from a git repository.",
        term::format::secondary("`rad init`")
    );

    Ok(())
}

pub fn authenticate(profiles: &[profile::Profile], options: Options) -> anyhow::Result<()> {
    let sock = keys::ssh_auth_sock();
    let profile = profile::default()?;

    if !options.active {
        term::info!(
            "Your active profile is {}",
            term::format::highlight(&profile.id().to_string()),
        );
    }

    let selection = if profiles.len() > 1 && !options.active {
        if let Some(p) = term::format::profile::select(profiles, &profile) {
            p
        } else {
            return Ok(());
        }
    } else {
        &profile
    };

    let read_only = profile::read_only(selection)?;
    let config = read_only.config()?;

    if let Some(user) = config.user()? {
        let username = config.user_name()?;

        term::headline(&format!(
            "🌱 Authenticating as {} {}",
            term::format::highlight(user),
            term::format::dim(format!("({})", username))
        ));
    }

    if selection.id() != profile.id() {
        let id = selection.id();
        profile::set(id)?;

        term::success!("Profile {} activated", id);
    }

    if !keys::is_ready(selection, sock.clone())? {
        term::warning("Adding your radicle key to ssh-agent...");

        // TODO: We should show the spinner on the passphrase prompt,
        // otherwise it seems like the passphrase is valid even if it isn't.
        let pass = term::pwhash(term::secret_input());
        let spinner = term::spinner("Unlocking...");

        keys::add(selection, pass, sock.clone()).context("invalid passphrase supplied")?;
        spinner.finish();

        term::success!("Radicle key added to ssh-agent");
    } else {
        term::success!("Signing key already in ssh-agent");
    }
    let (signer, _) = keys::storage(selection, sock)?;

    git::configure_signing(selection.paths().git_dir(), &signer.peer_id())?;
    term::success!("Signing key configured in git");

    Ok(())
}

pub fn check_ignored() -> anyhow::Result<()> {
    let profiles = ignored_profiles();
    if !profiles.is_empty() {
        term::warning(&format!(
            "Ignored empty profile(s): {:?}",
            &profiles
                .into_iter()
                .map(|p| p.id().to_string())
                .collect::<Vec<String>>()
        ));
    }

    Ok(())
}

pub fn ignored_profiles() -> Vec<profile::Profile> {
    filtered_profiles(|p| profile::read_only(p).is_err())
}

pub fn filtered_profiles<F>(func: F) -> Vec<profile::Profile>
where
    F: Fn(&profile::Profile) -> bool,
{
    match profile::list() {
        Ok(profiles) => profiles.into_iter().filter(|p| func(p)).collect(),
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use assay::assay;

    use super::*;

    use rad_common::test;

    fn create_auth_options(name: &str) -> Options {
        Options {
            active: false,
            init: true,
            name: Some(name.to_owned()),
            password: Some(test::USER_PASS.to_owned()),
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
}
