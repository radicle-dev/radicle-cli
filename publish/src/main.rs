use std::path::Path;

use rad_common::git;
use rad_publish::{DESCRIPTION, NAME, USAGE, VERSION};
use rad_terminal::compoments as term;

fn main() {
    term::run_command::<rad_sync::Options>("Publish", run);
}

fn run(options: rad_sync::Options) -> anyhow::Result<()> {
    if options.help {
        term::usage(NAME, VERSION, DESCRIPTION, USAGE);
        return Ok(());
    }
    term::info("Pushing 🌱 to remote `rad`");
    term::subcommand("git push rad");

    // Push to monorepo.
    match git::git(Path::new("."), ["push", "rad"]) {
        Ok(output) => term::blob(output),
        Err(err) => return Err(err),
    }
    // Sync monorepo to seed.
    rad_sync::run(options)?;

    Ok(())
}
