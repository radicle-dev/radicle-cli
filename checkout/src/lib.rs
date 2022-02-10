use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::git::Urn;

use rad_common::{identities, keys, profile};
use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

pub const HELP: Help = Help {
    name: "checkout",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad checkout <urn> [<option>...]

Options

    --help    Print help
"#,
};

pub struct Options {
    pub urn: Urn,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;
        use std::str::FromStr;

        let mut parser = lexopt::Parser::from_args(args);
        let mut urn = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => return Err(Error::Help.into()),
                Value(val) if urn.is_none() => {
                    let val = val.to_string_lossy();
                    let val = Urn::from_str(&val).context(format!("invalid URN '{}'", val))?;

                    urn = Some(val);
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                urn: urn.ok_or_else(|| anyhow!("a project URN to checkout must be provided"))?,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    execute(options).map(|_| ())
}

pub fn execute(options: Options) -> anyhow::Result<PathBuf> {
    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (signer, storage) = keys::storage(&profile, sock)?;
    let project = identities::project::get(&storage, &options.urn)?
        .context("project could not be found in local storage")?;
    let name = project.subject().name.to_string();
    let path = PathBuf::from(name.clone());

    if path.exists() {
        anyhow::bail!("the local path {:?} already exists", path.as_path());
    }

    term::headline(&format!(
        "Initializing local checkout for 🌱 {} ({})",
        term::format::highlight(&options.urn),
        name,
    ));

    let spinner = term::spinner("Performing checkout...");
    if let Err(err) = identities::project::checkout(
        &storage,
        profile.paths().clone(),
        signer,
        &options.urn,
        None,
        path.clone(),
    ) {
        spinner.failed();
        return Err(err.into());
    }
    spinner.finish();

    term::success!(
        "Project checkout successful under ./{}",
        term::format::highlight(name)
    );

    Ok(path)
}
