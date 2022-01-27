use rad_terminal::components::{Args, Error, Help};

pub const HELP: Help = Help {
    name: "show",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
USAGE
    rad show [OPTIONS]

OPTIONS
    --peer-id      Show device peer ID
    --project-id   Show current project ID
    --self         Show local user ID
    --help         Print help
"#,
};

#[derive(Default, Eq, PartialEq)]
pub struct Options {
    pub show_peer_id: bool,
    pub show_self: bool,
    pub show_proj_id: bool,
}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_env();
        let mut show_peer_id = false;
        let mut show_self = false;
        let mut show_proj_id = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("peer-id") => {
                    show_peer_id = true;
                }
                Long("self") => {
                    show_self = true;
                }
                Long("project-id") => {
                    show_proj_id = true;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok(Options {
            show_self,
            show_peer_id,
            show_proj_id,
        })
    }
}
