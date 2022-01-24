use std::path::Path;

use rad_common::git;
use rad_terminal::compoments as term;

const NAME: &str = "rad publish";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const DESCRIPTION: &str = "Publish radicle projects to the network";
const USAGE: &str = r#"
USAGE
    rad publish [--seed URL]

OPTIONS
    --seed URL    Use the given seed node for publishing
    --help        Print help
"#;

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
