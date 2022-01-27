use rad_terminal::components::{Args, Error, Help};

pub const HELP: Help = Help {
    name: "init",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
USAGE
    rad init [OPTIONS]

OPTIONS
    --help    Print help
"#,
};

pub struct Options {}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_env();

        if let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok(Options {})
    }
}
