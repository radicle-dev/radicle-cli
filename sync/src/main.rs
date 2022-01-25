use rad_sync::{run, Options, NAME};
use rad_terminal::compoments as term;

fn main() {
    term::run_command::<Options>(NAME, "Sync", run);
}
