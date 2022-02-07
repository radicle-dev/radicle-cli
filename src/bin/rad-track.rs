use rad_terminal::args;
use rad_track::HELP;
use rad_track::{run, Options};

fn main() {
    args::run_command::<Options, _>(HELP, "Tracking", run);
}
