use std::fmt;
use std::str::FromStr;

use zeroize::Zeroizing;

use librad::crypto::keystore::pinentry::SecUtf8;
use librad::crypto::BoxedSigner;
use librad::profile::Profile;

use dialoguer::{console::style, console::Style, theme::ColorfulTheme, Input, Password};

use radicle_common::cobs::issue::Issue;
use radicle_common::cobs::shared::CommentId;
use radicle_common::signer::ToSigner;

use super::command;
use super::format;
use super::keys;
use super::spinner::spinner;
use super::Error;

#[cfg(feature = "ethereum")]
pub use super::ethereum;

pub const TAB: &str = "   ";

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => ({
        println!("{}", format_args!($($arg)*));
    })
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => ({
        $crate::io::success_args(format_args!($($arg)*));
    })
}

#[macro_export]
macro_rules! tip {
    ($($arg:tt)*) => ({
        $crate::io::tip_args(format_args!($($arg)*));
    })
}

pub fn success_args(args: fmt::Arguments) {
    println!("{} {}", style("ok").green().reverse(), args);
}

pub fn tip_args(args: fmt::Arguments) {
    println!(
        "{} {}",
        style("=>").blue(),
        style(format!("{}", args)).dim()
    );
}

pub use info;
pub use success;
pub use tip;

pub fn width() -> usize {
    let (_, rows) = console::Term::stdout().size();
    rows as usize
}

pub fn headline(headline: &str) {
    println!();
    println!("{}", style(headline).bold());
    println!();
}

pub fn blob(text: impl fmt::Display) {
    println!("{}", style(text.to_string().trim()).dim());
}

pub fn blank() {
    println!()
}

pub fn print(msg: impl fmt::Display) {
    println!("{}", msg);
}

pub fn prefixed(prefix: &str, text: &str) -> String {
    text.split('\n')
        .map(|line| format!("{}{}\n", prefix, line))
        .collect()
}

pub fn help(name: &str, version: &str, description: &str, usage: &str) {
    println!("rad-{} {}\n{}\n{}", name, version, description, usage);
}

pub fn usage(name: &str, usage: &str) {
    eprintln!(
        "{} {}\n{}",
        style("==").red(),
        style(format!("Error: rad-{}: invalid usage", name)).red(),
        style(prefixed(TAB, usage)).red().dim()
    );
}

pub fn eprintln(prefix: impl fmt::Display, msg: impl fmt::Display) {
    eprintln!("{} {}", prefix, msg);
}

pub fn indented(msg: &str) {
    println!("{}{}", TAB, msg);
}

pub fn subcommand(msg: impl fmt::Display) {
    println!("{} {}", style("$").dim(), style(msg).dim());
}

pub fn warning(warning: &str) {
    eprintln!(
        "{} {} {}",
        style("**").yellow(),
        style("Warning:").yellow().bold(),
        style(warning).yellow()
    );
}

pub fn error(error: impl fmt::Display) {
    eprintln!("{} {}", style("==").red(), style(error).red());
}

pub fn fail(header: &str, error: &anyhow::Error) {
    let err = error.to_string();
    let err = err.trim_end();
    let separator = if err.len() > 160 || err.contains('\n') {
        "\n"
    } else {
        " "
    };

    eprintln!(
        "{} {}{}{}",
        style("==").red(),
        style(header).red().reverse(),
        separator,
        style(error).red().bold(),
    );

    let cause = error.root_cause();
    if cause.to_string() != error.to_string() {
        eprintln!(
            "{} {}",
            style("==").red().dim(),
            style(error.root_cause()).red().dim()
        );
        blank();
    }

    if let Some(Error::WithHint { hint, .. }) = error.downcast_ref::<Error>() {
        eprintln!("{} {}", style("==").yellow(), style(hint).yellow(),);
        blank();
    }
}

pub fn ask<D: fmt::Display>(prompt: D, default: bool) -> bool {
    dialoguer::Confirm::new()
        .with_prompt(format!("{} {}", style(" ⤷".to_owned()).cyan(), prompt))
        .wait_for_newline(false)
        .default(true)
        .default(default)
        .interact()
        .unwrap_or_default()
}

pub fn confirm<D: fmt::Display>(prompt: D) -> bool {
    ask(format::tertiary(prompt), true)
}

pub fn abort<D: fmt::Display>(prompt: D) -> bool {
    ask(format::tertiary(prompt), false)
}

/// Get the signer. First we try getting it from ssh-agent, otherwise we prompt the user.
pub fn signer(profile: &Profile) -> anyhow::Result<BoxedSigner> {
    let signer = if let Ok(sock) = keys::ssh_auth_sock() {
        sock.to_signer(profile)?
    } else {
        secret_key(profile)?.to_signer(profile)?
    };
    Ok(signer)
}

pub fn theme() -> ColorfulTheme {
    ColorfulTheme {
        success_prefix: style("ok".to_owned()).for_stderr().green().reverse(),
        prompt_prefix: style(" ⤷".to_owned()).cyan().dim().for_stderr(),
        prompt_suffix: style("·".to_owned()).cyan().for_stderr(),
        prompt_style: Style::new().cyan().bold().for_stderr(),
        active_item_style: Style::new().for_stderr().yellow().reverse(),
        active_item_prefix: style("*".to_owned()).yellow().for_stderr(),
        picked_item_prefix: style("*".to_owned()).yellow().for_stderr(),
        inactive_item_prefix: style(" ".to_string()).for_stderr(),
        inactive_item_style: Style::new().yellow().for_stderr(),
        error_prefix: style("⤹  Error:".to_owned()).red().for_stderr(),
        success_suffix: style("·".to_owned()).cyan().for_stderr(),

        ..ColorfulTheme::default()
    }
}

pub fn text_input<S, E>(message: &str, default: Option<S>) -> anyhow::Result<S>
where
    S: fmt::Display + std::str::FromStr<Err = E> + Clone,
    E: fmt::Debug + fmt::Display,
{
    let theme = theme();
    let mut input: Input<S> = Input::with_theme(&theme);

    let value = match default {
        Some(default) => input
            .with_prompt(message)
            .with_initial_text(default.to_string())
            .interact_text()?,
        None => input.with_prompt(message).interact_text()?,
    };
    Ok(value)
}

#[derive(Debug, Default, Clone)]
pub struct Optional<T> {
    option: Option<T>,
}

impl<T: fmt::Display> fmt::Display for Optional<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(val) = &self.option {
            write!(f, "{}", val)
        } else {
            write!(f, "")
        }
    }
}

impl<T: FromStr> FromStr for Optional<T> {
    type Err = <T as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Ok(Optional { option: None });
        }
        let val: T = s.parse()?;

        Ok(Self { option: Some(val) })
    }
}

pub fn text_input_optional<S, E>(
    message: &str,
    initial: Option<String>,
) -> anyhow::Result<Option<S>>
where
    S: fmt::Display + fmt::Debug + FromStr<Err = E> + Clone,
    E: fmt::Debug + fmt::Display,
{
    let theme = theme();
    let mut input: Input<Optional<S>> = Input::with_theme(&theme);

    if let Some(init) = initial {
        input.with_initial_text(init);
    }
    let value = input
        .with_prompt(message)
        .allow_empty(true)
        .interact_text()?;

    Ok(value.option)
}

pub fn secret_input() -> SecUtf8 {
    secret_input_with_prompt("Passphrase")
}

// TODO: This prompt shows success just for entering a password,
// even if the password is later found out to be wrong.
// We should handle this differently.
pub fn secret_input_with_prompt(prompt: &str) -> SecUtf8 {
    SecUtf8::from(
        Password::with_theme(&theme())
            .allow_empty_password(true)
            .with_prompt(prompt)
            .interact()
            .unwrap(),
    )
}

pub fn secret_input_with_confirmation() -> SecUtf8 {
    SecUtf8::from(
        Password::with_theme(&theme())
            .with_prompt("Passphrase")
            .with_confirmation("Repeat passphrase", "Error: the passphrases don't match.")
            .interact()
            .unwrap(),
    )
}

pub fn secret_stdin() -> Result<SecUtf8, anyhow::Error> {
    let mut input: Zeroizing<String> = Zeroizing::new(Default::default());
    std::io::stdin().read_line(&mut input)?;

    Ok(SecUtf8::from(input.trim_end()))
}

pub fn read_passphrase(stdin: bool, confirm: bool) -> Result<SecUtf8, anyhow::Error> {
    let passphrase = match read_passphrase_from_env_var() {
        Ok(input) => input,
        _ => {
            if stdin {
                secret_stdin()?
            } else if confirm {
                secret_input_with_confirmation()
            } else {
                secret_input()
            }
        }
    };

    Ok(passphrase)
}

pub fn read_passphrase_from_env_var() -> Result<SecUtf8, anyhow::Error> {
    let env_var = std::env::var(keys::RAD_PASSPHRASE)?;
    let input: Zeroizing<String> = Zeroizing::new(env_var);

    Ok(SecUtf8::from(input.trim_end()))
}

pub fn secret_key(profile: &Profile) -> Result<keys::signer::ZeroizingSecretKey, anyhow::Error> {
    let passphrase = secret_input();
    let spinner = spinner("Unsealing key..."); // Nb. Spinner ends when dropped.
    let key = keys::load_secret_key(profile, passphrase)?;

    spinner.finish();

    Ok(key)
}

pub fn select<'a, T>(options: &'a [T], active: &'a T) -> Option<&'a T>
where
    T: fmt::Display + Eq + PartialEq,
{
    let theme = theme();
    let active = options.iter().position(|o| o == active);
    let mut selection = dialoguer::Select::with_theme(&theme);

    if let Some(active) = active {
        selection.default(active);
    }
    let result = selection
        .items(&options.iter().map(|p| p.to_string()).collect::<Vec<_>>())
        .interact_opt()
        .unwrap();

    result.map(|i| &options[i])
}

pub fn select_with_prompt<'a, T>(prompt: &str, options: &'a [T], active: &'a T) -> Option<&'a T>
where
    T: fmt::Display + Eq + PartialEq,
{
    let theme = theme();
    let active = options.iter().position(|o| o == active);
    let mut selection = dialoguer::Select::with_theme(&theme);
    selection.with_prompt(prompt);

    if let Some(active) = active {
        selection.default(active);
    }
    let result = selection
        .items(&options.iter().map(|p| p.to_string()).collect::<Vec<_>>())
        .interact_opt()
        .unwrap();

    result.map(|i| &options[i])
}

pub fn profile_select<'a>(profiles: &'a [Profile], active: &Profile) -> Option<&'a Profile> {
    let active = profiles.iter().position(|p| p.id() == active.id()).unwrap();
    let selection = dialoguer::Select::with_theme(&theme())
        .items(&profiles.iter().map(|p| p.id()).collect::<Vec<_>>())
        .default(active)
        .interact_opt()
        .unwrap();

    selection.map(|i| &profiles[i])
}

pub fn comment_select(issue: &Issue) -> Option<CommentId> {
    let selection = dialoguer::Select::with_theme(&theme())
        .with_prompt("Which comment do you want to react to?")
        .item(&issue.description().to_string())
        .items(
            &issue
                .comments()
                .iter()
                .map(|p| p.body.clone())
                .collect::<Vec<_>>(),
        )
        .default(CommentId::root().into())
        .interact_opt()
        .unwrap();

    selection.map(CommentId::from)
}

pub fn markdown(content: &str) {
    if !content.is_empty() {
        if command::bat(["-p", "-l", "md"], content).is_err() {
            blob(content);
        }
    }
}

fn _info(args: std::fmt::Arguments) {
    println!("{}", args);
}
