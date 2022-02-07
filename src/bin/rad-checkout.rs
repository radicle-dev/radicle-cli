use rad_checkout::{run, Options, HELP};
use rad_terminal::args;

fn main() {
    args::run_command::<Options, _>(HELP, "Project checkout", run);
}
