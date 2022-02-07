use rad_account::{run, Options, HELP};
use rad_terminal::args;

fn main() {
    args::run_command::<Options, _>(HELP, "Command", move |opts| {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(run(opts))
    });
}
