use rad_terminal::args;
use rad_tree::{run, Options, HELP};

fn main() {
    args::run_command::<Options, _>(HELP, "Command", run);
}
