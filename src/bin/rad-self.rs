use rad_self::{run, Options, HELP};

fn main() {
    rad_terminal::args::run_command::<Options, _>(HELP, "Command", run);
}
