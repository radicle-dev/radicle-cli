use rad_terminal::components::Help;

pub const HELP: Help = Help {
    name: "push",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
USAGE
    rad push [--seed <host>] [--http]

OPTIONS
    --seed <host>    Use the given seed node for syncing
    --http           Use HTTP instead of HTTPS for publishing (default: false)
    --help           Print help
"#,
};
