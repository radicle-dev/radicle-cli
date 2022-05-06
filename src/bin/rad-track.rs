use rad_terminal as term;
use rad_track::HELP;
use rad_track::{run, Options};

fn main() {
    term::run_command::<Options, _>(HELP, "Command", run);
}
