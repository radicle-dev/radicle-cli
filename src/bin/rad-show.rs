use rad_show::{run, Options, HELP};

fn main() {
    rad_terminal::args::run_command::<Options, _>(HELP, "Show", run);
}
