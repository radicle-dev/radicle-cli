// TODO: Allow default seed configuration in .gitconfig
// TODO: Push to rad master on publish

use rad_sync::{run, Options};
use rad_terminal::compoments as term;

fn main() {
    term::run_command::<Options>("Sync", run);
}
