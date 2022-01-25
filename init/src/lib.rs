use rad_terminal::compoments::Args;

pub const NAME: &str = "init";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DESCRIPTION: &str = "Initialize radicle projects from git repositories";
pub const USAGE: &str = r#"
USAGE
    rad init [OPTIONS]

OPTIONS
    --help    Print help
"#;

pub struct Options {
    pub help: bool,
}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_env();
        let mut help = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => help = true,
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok(Options { help })
    }
}
