use rad_init::{run, Options, HELP};
use rad_terminal as term;

fn main() {
    term::run_command::<Options, _>(HELP, "Initialization", run);
}
