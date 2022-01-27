use rad_terminal::components::Help;

pub const HELP: Help = Help {
    name: "publish",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
USAGE
    rad publish [--seed URL]

OPTIONS
    --seed URL    Use the given seed node for publishing
    --help        Print help
"#,
};
