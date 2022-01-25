use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::git::Urn;

use rad_common::{identities, keys, profile};
use rad_terminal::compoments as term;
use rad_terminal::compoments::Args;

const NAME: &str = "rad checkout";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const DESCRIPTION: &str = "Checkout a project working copy";
const USAGE: &str = r#"
USAGE
    rad checkout <urn> [<option>...]

OPTIONS
    --help    Print help
"#;

pub struct Options {
    help: bool,
    urn: Urn,
}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        use lexopt::prelude::*;
        use std::str::FromStr;

        let mut parser = lexopt::Parser::from_env();
        let mut help = false;
        let mut urn = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => help = true,
                Value(val) if urn.is_none() => {
                    let val = val.to_string_lossy();
                    let val = Urn::from_str(&val).context(format!("invalid URN '{}'", val))?;

                    urn = Some(val);
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok(Options {
            help,
            urn: urn.ok_or_else(|| anyhow!("a project URN to checkout must be provided"))?,
        })
    }
}

fn main() {
    term::run_command::<Options>("Project checkout", run);
}

fn run(options: Options) -> anyhow::Result<()> {
    if options.help {
        term::usage(NAME, VERSION, DESCRIPTION, USAGE);
        return Ok(());
    }

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
        path,
    ) {
        spinner.failed();
        return Err(err.into());
    }
    spinner.finish();

    term::success(&format!(
        "Project checkout successful under ./{}",
        term::format::highlight(name)
    ));

    Ok(())
}
