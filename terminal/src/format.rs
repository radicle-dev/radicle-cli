pub use dialoguer::console::style;

pub fn negative<D: std::fmt::Display>(msg: D) -> String {
    style(msg).red().bright().to_string()
}

pub fn positive<D: std::fmt::Display>(msg: D) -> String {
    style(msg).green().bright().to_string()
}

pub fn secondary<D: std::fmt::Display>(msg: D) -> String {
    style(msg).blue().bright().to_string()
}

pub fn tertiary<D: std::fmt::Display>(msg: D) -> String {
    style(msg).cyan().to_string()
}

pub fn tertiary_bold<D: std::fmt::Display>(msg: D) -> String {
    style(msg).cyan().bold().to_string()
}

pub fn yellow<D: std::fmt::Display>(msg: D) -> String {
    style(msg).yellow().to_string()
}

pub fn highlight<D: std::fmt::Display>(input: D) -> String {
    style(input).green().bright().to_string()
}

pub fn badge_primary<D: std::fmt::Display>(input: D) -> String {
    style(format!(" {} ", input))
        .magenta()
        .reverse()
        .to_string()
}

pub fn badge_positive<D: std::fmt::Display>(input: D) -> String {
    style(format!(" {} ", input)).green().reverse().to_string()
}

pub fn badge_negative<D: std::fmt::Display>(input: D) -> String {
    style(format!(" {} ", input)).red().reverse().to_string()
}

pub fn badge_secondary<D: std::fmt::Display>(input: D) -> String {
    style(format!(" {} ", input)).blue().reverse().to_string()
}

pub fn bold<D: std::fmt::Display>(input: D) -> String {
    style(input).white().bright().bold().to_string()
}

pub fn dim<D: std::fmt::Display>(input: D) -> String {
    style(input).dim().to_string()
}

pub fn italic<D: std::fmt::Display>(input: D) -> String {
    style(input).italic().dim().to_string()
}
