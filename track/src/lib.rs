use rad_terminal::components::Help;

pub mod options;

pub const HELP: Help = Help {
    name: "track",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
USAGE
    rad track <urn> [--peer <peer-id>]

OPTIONS
    --peer <peer-id>   Peer ID to track (default: all)
    --help             Print help
"#,
};
