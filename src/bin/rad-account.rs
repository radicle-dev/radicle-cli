use rad_account::{run, Options, HELP};
use radicle_terminal as term;

fn main() {
    term::run_command::<Options, _>(HELP, "Command", move |opts, ctx| {
        let rt = radicle_common::tokio::runtime::Runtime::new()?;
        rt.block_on(run(opts, ctx))
    });
}
