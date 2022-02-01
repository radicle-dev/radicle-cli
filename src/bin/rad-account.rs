use rad_account::{run, Options, HELP};
use rad_terminal::components as term;

fn main() {
    term::run_command::<Options, _>(HELP, "Command", move |opts| {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(run(opts))
    });
}
