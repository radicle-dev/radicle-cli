use std::path::Path;

use rad_common::{git, profile};
use rad_push::HELP;
use rad_terminal::args;
use rad_terminal::components as term;

fn main() {
    args::run_command::<rad_sync::Options, _>(HELP, "Push", run);
}

fn run(options: rad_sync::Options) -> anyhow::Result<()> {
    profile::default()?;

    if options.fetch {
        anyhow::bail!("option `--fetch` cannot be used when pushing");
    }
    term::info!("Pushing 🌱 to remote `rad`");

    let args = if options.force {
        term::subcommand("git push --force rad");
        vec!["push", "--force", "rad"]
    } else {
        term::subcommand("git push rad");
        vec!["push", "rad"]
    };

    // Push to monorepo.
    match git::git(Path::new("."), args) {
        Ok(output) => term::blob(output),
        Err(err) => return Err(err),
    }
    // Sync monorepo to seed.
    rad_sync::run(options)?;

    Ok(())
}
