use rad_patch::{run, HELP};
use radicle_terminal as term;

fn main() {
    term::run_command::<rad_patch::Options, _>(HELP, "Patch", run);
}
