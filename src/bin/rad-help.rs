use rad_terminal::args;

fn main() {
    args::run_command::<rad_help::Options, _>(rad_help::HELP, "Help", rad_help::run);
}
