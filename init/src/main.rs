// TODO: Take default branch from current git branch
use anyhow::bail;

use librad::canonical::Cstring;
use librad::identities::payload::{self};

use rad_common::{keys, profile, project};
use rad_terminal::compoments as term;
use rad_terminal::compoments::Args;

const NAME: &str = "rad init";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const DESCRIPTION: &str = "Initialize radicle projects from git repositories";
const USAGE: &str = r#"
USAGE
    rad init [OPTIONS]

OPTIONS
    --help    Print help
"#;

pub struct Options {
    help: bool,
}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_env();
        let mut help = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => help = true,
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok(Options { help })
    }
}

fn main() {
    term::run_command::<Options>("Project initialization", run);
}

fn run(options: Options) -> anyhow::Result<()> {
    if options.help {
        term::usage(NAME, VERSION, DESCRIPTION, USAGE);
        return Ok(());
    }

    let cwd = std::env::current_dir()?;
    let path = cwd.as_path();
    let name = path.file_name().unwrap().to_string_lossy().to_string();

    term::headline(&format!(
        "Initializing local 🌱 project {}",
        term::format::highlight(&name)
    ));

    let _repo = project::repository()?;
    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (signer, storage) = keys::storage(&profile, sock)?;

    let description = term::text_input("Description", None);
    let branch = term::text_input("Default branch", Some("master".to_string()));

    let spinner = term::spinner(&format!(
        "Initializing new project in {}...",
        path.display()
    ));

    let payload = payload::Project {
        name: Cstring::from(name),
        description: Some(Cstring::from(description)),
        default_branch: Some(Cstring::from(branch.clone())),
    };

    match project::create(&storage, signer, &profile, payload) {
        Ok(proj) => {
            let urn = proj.urn();

            spinner.finish();

            term::success(&format!(
                "Project initialized: {}",
                term::format::highlight(&urn.to_string())
            ));
            term::blank();
            term::info(&format!(
                "To publish, run `rad publish` or `git push rad {}`",
                branch
            ));
        }
        Err(err) => {
            spinner.finish();

            use rad_common::identities::git::existing::Error;
            use rad_common::identities::git::validation;

            match err.downcast_ref::<Error>() {
                Some(Error::Validation(validation::Error::UrlMismatch { found, .. })) => {
                    bail!(
                        "this repository is already initialized with remote {}",
                        found
                    );
                }
                Some(Error::Validation(validation::Error::MissingDefaultBranch { .. })) => bail!(
                    "the `{}` branch was either not found, or has no commits",
                    branch
                ),
                Some(_) | None => return Err(err),
            }
        }
    }

    Ok(())
}
