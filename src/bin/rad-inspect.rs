use rad_inspect::{run, Options, HELP};

fn main() {
    rad_terminal::args::run_command::<Options, _>(HELP, "Inspect", run);
}
