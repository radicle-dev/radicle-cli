use rad_terminal::compoments as term;
use rad_terminal::compoments::Args;

const COMMANDS: &[(&str, &str)] = &[
    (rad_auth::NAME, rad_auth::DESCRIPTION),
    (rad_init::NAME, rad_init::DESCRIPTION),
    (rad_publish::NAME, rad_publish::DESCRIPTION),
    (rad_checkout::NAME, rad_checkout::DESCRIPTION),
    (rad_track::NAME, rad_track::DESCRIPTION),
    (rad_untrack::NAME, rad_untrack::DESCRIPTION),
    (rad_sync::NAME, rad_sync::DESCRIPTION),
    (rad_help::NAME, rad_help::DESCRIPTION),
];

struct Options {}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        Ok(Options {})
    }
}

fn main() {
    term::run_command::<Options>("Help", run);
}

fn run(_options: Options) -> anyhow::Result<()> {
    println!("Usage: rad <command> [--help]");

    if rad_common::profile::default().is_err() {
        println!();
        println!(
            "{}",
            term::format::highlight("It looks like this is your first time using radicle.")
        );
        println!(
            "{}",
            term::format::highlight("To get started, use `rad auth` to authenticate.")
        );
        println!();
    }

    println!("Common `rad` commands used in various situations:");
    println!();

    for (name, description) in COMMANDS {
        println!(
            "\t{} {}",
            term::format::bold(format!("{:-12}", name)),
            term::format::dim(description)
        );
    }
    println!();
    println!("See `rad <command> --help` to learn about a specific command.");
    println!();

    Ok(())
}
