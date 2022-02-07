use rad_ens::{run, Options, HELP};
use rad_terminal::args;

fn main() {
    args::run_command::<Options, _>(HELP, "Command", run);
}
