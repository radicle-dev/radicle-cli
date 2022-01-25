use rad_terminal::compoments as term;

fn main() {
    term::run_command::<rad_help::Options>(rad_help::NAME, "Help", rad_help::run);
}
