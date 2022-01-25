use rad_terminal::compoments as term;
use rad_terminal::compoments::Args;

pub const NAME: &str = "rad help";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DESCRIPTION: &str = "Print radicle tools help";
pub const USAGE: &str = r#"
USAGE
    Common `rad` commands used in various situations:
"#;

const COMMANDS: &[(&str, &str)] = &[(rad_auth::NAME, rad_auth::DESCRIPTION)];

struct Options {}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        Ok(Options {})
    }
}

fn main() {
    term::run_command::<Options>("Help", run);
}

fn run(_options: Options) -> anyhow::Result<()> {
    for _command in COMMANDS {}

    Ok(())
}
