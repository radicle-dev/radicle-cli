use rad_push::{run, Options, HELP};
use radicle_terminal as term;

// TODO: Pass all options after `--` to git.
fn main() {
    term::run_command::<Options, _>(HELP, "Push", run);
}
