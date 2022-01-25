use anyhow::anyhow;
use anyhow::Context as _;

use librad::git::Urn;

use rad_terminal::compoments::Args;

pub const NAME: &str = "checkout";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DESCRIPTION: &str = "Checkout a project working copy";
pub const USAGE: &str = r#"
USAGE
    rad checkout <urn> [<option>...]

OPTIONS
    --help    Print help
"#;

pub struct Options {
    pub help: bool,
    pub urn: Urn,
}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        use lexopt::prelude::*;
        use std::str::FromStr;

        let mut parser = lexopt::Parser::from_env();
        let mut help = false;
        let mut urn = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => help = true,
                Value(val) if urn.is_none() => {
                    let val = val.to_string_lossy();
                    let val = Urn::from_str(&val).context(format!("invalid URN '{}'", val))?;

                    urn = Some(val);
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok(Options {
            help,
            urn: urn.ok_or_else(|| anyhow!("a project URN to checkout must be provided"))?,
        })
    }
}
