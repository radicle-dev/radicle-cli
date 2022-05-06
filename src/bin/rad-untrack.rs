use rad_terminal as term;
use rad_untrack::{run, Options, HELP};

fn main() {
    term::run_command::<Options, _>(HELP, "Untracking", run);
}
