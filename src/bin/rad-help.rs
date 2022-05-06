use rad_terminal as term;

fn main() {
    term::run_command::<rad_help::Options, _>(rad_help::HELP, "Help", rad_help::run);
}
