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

    let mut args = vec!["push"];

    if options.force {
        args.push("--force");
    }
    if options.set_upstream {
        args.push("--set-upstream");
    }
    if options.all {
        args.push("--all");
    }
    if options.verbose {
        args.push("--verbose");
    }
    args.push("rad"); // Push to "rad" remote.

    term::subcommand(&format!("git {}", args.join(" ")));

    // Push to monorepo.
    match git::git(Path::new("."), args) {
        Ok(output) => term::blob(output),
        Err(err) => return Err(err),
    }

    if options.sync {
        // Sync monorepo to seed.
        rad_sync::run(rad_sync::Options {
            refs: if options.all {
                rad_sync::Refs::All
            } else if let Some(head) = head {
                rad_sync::Refs::Branch(head)
            } else {
                anyhow::bail!("You must be on a branch in order to push");
            },
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
