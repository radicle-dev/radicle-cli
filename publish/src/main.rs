use std::path::Path;

use rad_common::git;
use rad_terminal::compoments as term;

fn main() -> anyhow::Result<()> {
    let options = rad_sync::Options::from_env()?;

    match run(options) {
        Ok(()) => Ok(()),
        Err(err) => {
            term::format::error("Publish failed", &err);
            std::process::exit(1);
        }
    }
}

fn run(options: rad_sync::Options) -> anyhow::Result<()> {
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
