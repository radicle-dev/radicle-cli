use anyhow::anyhow;
use anyhow::Context as _;

use librad::git::Urn;

use rad_terminal::components::{Args, Error, Help};

pub const HELP: Help = Help {
    name: "checkout",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
USAGE
    rad checkout <urn> [<option>...]

OPTIONS
    --help    Print help
"#,
};

pub struct Options {
    pub urn: Urn,
}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        use lexopt::prelude::*;
        use std::str::FromStr;

        let mut parser = lexopt::Parser::from_env();
        let mut urn = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => return Err(Error::Help.into()),
                Value(val) if urn.is_none() => {
                    let val = val.to_string_lossy();
                    let val = Urn::from_str(&val).context(format!("invalid URN '{}'", val))?;

                    urn = Some(val);
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok(Options {
            urn: urn.ok_or_else(|| anyhow!("a project URN to checkout must be provided"))?,
        })
    }
}
