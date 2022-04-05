use rad_patch::{run, HELP};
use rad_terminal::args;

fn main() {
    args::run_command::<rad_patch::Options, _>(HELP, "Patch", run);
}
