use rad_terminal::components::{Args, Error, Help};

pub const HELP: Help = Help {
    name: "auth",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
USAGE
    rad auth [--init]

OPTIONS
    --init    Initialize a new identity
    --help    Print help
"#,
};

#[derive(Debug)]
pub struct Options {
    pub init: bool,
}

impl Args for Options {
    fn from_env() -> Result<Self, anyhow::Error> {
        use lexopt::prelude::*;

        let mut init = false;
        let mut parser = lexopt::Parser::from_env();

        while let Some(arg) = parser.next()? {
            match arg {
                Long("init") => {
                    init = true;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok(Options { init })
    }
}
