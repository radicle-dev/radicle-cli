use rad_clone::{run, Options, HELP};
use rad_terminal::args;

fn main() {
    args::run_command::<Options, _>(HELP, "Cloning", run);
}
