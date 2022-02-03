use rad_terminal::components as term;
use rad_track::options::Options;
use rad_untrack::{run, HELP};

fn main() {
    term::run_command::<Options, _>(HELP, "Untracking", run);
}
