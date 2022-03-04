use std::path::Path;

use rad_common::{git, profile};
use rad_push::HELP;
use rad_terminal::args;
use rad_terminal::components as term;

// TODO: Pass all options after `--` to git.
fn main() {
    args::run_command::<rad_push::Options, _>(HELP, "Push", run);
}

fn run(options: rad_push::Options) -> anyhow::Result<()> {
    profile::default()?;

    term::info!("Pushing 🌱 to remote `rad`");

    let repo = git::Repository::open(Path::new("."))?;
    let head: Option<String> = repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().map(|h| h.to_owned()));

    let mut args = if options.force {
        vec!["push", "-u", "--force", "rad"]
    } else {
        vec!["push", "-u", "rad"]
    };

    if options.verbose {
        args.push("--verbose");
    }
    term::subcommand(&format!("git {}", args.join(" ")));

    // Push to monorepo.
    match git::git(Path::new("."), args) {
        Ok(output) => term::blob(output),
        Err(err) => return Err(err),
    }

    if options.sync {
        // Sync monorepo to seed.
        rad_sync::run(rad_sync::Options {
            head: if options.all { None } else { head },
            all: options.all,
            seed: options.seed,
            identity: options.identity,
            verbose: options.verbose,

            fetch: false,
            origin: None,
            push_self: false,
        })?;
    }

    Ok(())
}
