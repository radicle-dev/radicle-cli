// TODO: Allow default seed configuration in .gitconfig
// TODO: Push to rad master on publish

use rad_sync::{run, Options};
use rad_terminal::compoments as term;

fn main() -> anyhow::Result<()> {
    let options = Options::from_env()?;

    match run(options) {
        Ok(()) => Ok(()),
        Err(err) => {
            term::format::error("Sync failed", &err);
            std::process::exit(1);
        }
    }
}
