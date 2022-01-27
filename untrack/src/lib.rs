use rad_terminal::components::Help;

pub const NAME: &str = "untrack";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DESCRIPTION: &str = "Untrack project peers";
pub const USAGE: &str = r#"
USAGE
    rad untrack <urn> [--peer <peer-id>]

OPTIONS
    --peer <peer-id>   Peer ID to track (default: all)
    --help             Print help
"#;

pub const HELP: Help = Help {
    name: NAME,
    description: DESCRIPTION,
    version: VERSION,
    usage: USAGE,
};
