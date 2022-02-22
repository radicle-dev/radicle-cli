use rad_ci::{run, Options, HELP};

fn main() {
    rad_terminal::args::run_command::<Options, _>(HELP, "ci", run);
}
