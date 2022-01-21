use rad_terminal::compoments::Args;

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
                    println!("Usage: rad auth [--init]");
                    std::process::exit(0);
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok(Options { init })
    }
}
