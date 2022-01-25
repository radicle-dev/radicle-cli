pub mod options;

pub const NAME: &str = "track";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DESCRIPTION: &str = "Track project peers";
pub const USAGE: &str = r#"
USAGE
    rad track <urn> [--peer <peer-id>]

OPTIONS
    --peer <peer-id>   Peer ID to track (default: all)
    --help             Print help
"#;
