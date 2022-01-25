use rad_terminal::compoments as term;

fn main() {
    term::run_command::<rad_help::Options>("Help", rad_help::run);
}
