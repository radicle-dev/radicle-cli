use rad_terminal::components as term;

fn main() {
    term::run_command::<rad_help::Options>(rad_help::HELP, "Help", rad_help::run);
}
