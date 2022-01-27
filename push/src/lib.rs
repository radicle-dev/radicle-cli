use rad_terminal::components::Help;

pub const HELP: Help = Help {
    name: "push",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
USAGE
    rad push [--seed URL]

OPTIONS
    --seed URL    Use the given seed node for syncing
    --help        Print help
"#,
};
