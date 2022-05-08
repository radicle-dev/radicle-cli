use rad_track::HELP;
use rad_track::{run, Options};
use radicle_terminal as term;

fn main() {
    term::run_command::<Options, _>(HELP, "Command", run);
}
