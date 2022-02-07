use rad_terminal::args;
use rad_track::Options;
use rad_untrack::{run, HELP};

fn main() {
    args::run_command::<Options, _>(HELP, "Untracking", run);
}
