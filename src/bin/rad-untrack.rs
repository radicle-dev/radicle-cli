use rad_terminal::args;
use rad_untrack::{run, Options, HELP};

fn main() {
    args::run_command::<Options, _>(HELP, "Untracking", run);
}
