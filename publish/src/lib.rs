pub const NAME: &str = "publish";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DESCRIPTION: &str = "Publish radicle projects to the network";
pub const USAGE: &str = r#"
USAGE
    rad publish [--seed URL]

OPTIONS
    --seed URL    Use the given seed node for publishing
    --help        Print help
"#;
