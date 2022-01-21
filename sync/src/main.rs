// TODO: Allow default seed configuration in .gitconfig
// TODO: Push to rad master on publish

use rad_sync::{run, Options};
use rad_terminal::compoments as term;

fn main() -> anyhow::Result<()> {
    let options = match Options::from_env() {
        Ok(opts) => opts,
        Err(err) => {
            term::failure(&err);
            std::process::exit(1);
        }
    };

    match run(options) {
        Ok(()) => Ok(()),
        Err(err) => {
            term::format::error("Sync failed", &err);
            std::process::exit(1);
        }
    }
}
