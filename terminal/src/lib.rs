pub mod keys {
    use librad::crypto::keystore::pinentry::{Pinentry, SecUtf8};

    #[derive(Clone)]
    pub struct CachedPrompt(pub SecUtf8);

    impl CachedPrompt {
        pub fn new(secret: SecUtf8) -> Self {
            Self(secret)
        }
    }

    impl Pinentry for CachedPrompt {
        type Error = std::io::Error;

        fn get_passphrase(&self) -> Result<SecUtf8, Self::Error> {
            Ok(self.0.clone())
        }
    }
}

pub mod compoments {
    use std::{fmt, process};

    use librad::crypto::keystore::crypto::{KdfParams, Pwhash};
    use librad::crypto::keystore::pinentry::SecUtf8;

    use dialoguer::{console::style, console::Style, theme::ColorfulTheme, Input, Password};
    use indicatif::{ProgressBar, ProgressFinish, ProgressStyle};

    use super::keys;

    pub trait Args: Sized {
        fn from_env() -> anyhow::Result<Self>;
    }

    pub struct Spinner {
        progress: ProgressBar,
        message: String,
    }

    impl Spinner {
        pub fn finish(self) {
            self.progress.finish_and_clear();
            self::success(&self.message);
        }

        pub fn failed(self) {
            self.progress.finish_and_clear();
            self::eprintln(style("--").red(), self.message);
        }
    }

    pub fn run_command<A>(name: &str, run: fn(A) -> anyhow::Result<()>) -> !
    where
        A: Args,
    {
        let options = match A::from_env() {
            Ok(opts) => opts,
            Err(err) => {
                self::failure(&err);
                process::exit(1);
            }
        };

        match run(options) {
            Ok(()) => process::exit(0),
            Err(err) => {
                self::format::error(&format!("{} failed", name), &err);
                process::exit(1);
            }
        }
    }

    pub fn headline(headline: &str) {
        println!();
        println!("{}", style(headline).bold());
        println!();
    }

    pub fn blob(text: impl fmt::Display) {
        println!("{}", style(text).dim());
    }

    pub fn blank() {
        println!()
    }

    pub fn prefixed(prefix: &str, text: &str) -> String {
        text.split('\n')
            .filter(|line| !line.trim().is_empty())
            .map(|line| format!("  {}: {}\n", prefix, line))
            .collect()
    }

    pub fn usage(name: &str, version: &str, description: &str, usage: &str) {
        println!("{} v{}\n{}\n{}", name, version, description, usage);
    }

    pub fn eprintln(prefix: impl fmt::Display, msg: impl fmt::Display) {
        eprintln!("{} {}", prefix, msg);
    }

    pub fn info(info: &str) {
        println!("{} {}", style("=>").blue(), info);
    }

    pub fn subcommand(msg: &str) {
        println!("{} {}", style("$").dim(), style(msg).dim());
    }

    pub fn warning(warning: &str) {
        eprintln!("{} {}", style("**").yellow(), style(warning).yellow());
    }

    pub fn error(error: &str) {
        eprintln!("{} {}", style("==").red(), style(error).red());
    }

    pub fn success(success: &str) {
        println!("{} {}", style("OK").green(), success);
    }

    pub fn failure(error: &anyhow::Error) {
        eprintln!(
            "{} {} {}",
            style("==").red(),
            style("Error:").red(),
            style(error).red()
        );
    }

    pub fn spinner(message: &str) -> Spinner {
        let message = message.to_owned();
        let style = ProgressStyle::default_spinner()
            .tick_strings(&[
                &style("\\ ").yellow().to_string(),
                &style("| ").yellow().to_string(),
                &style("/ ").yellow().to_string(),
                &style("| ").yellow().to_string(),
            ])
            .template("{spinner} {msg}")
            .on_finish(ProgressFinish::AndClear);

        let progress = ProgressBar::new(!0);
        progress.set_style(style);
        progress.enable_steady_tick(99);
        progress.set_message(message.clone());

        Spinner { message, progress }
    }

    pub fn pwhash(secret: SecUtf8) -> Pwhash<keys::CachedPrompt> {
        let prompt = keys::CachedPrompt::new(secret);
        Pwhash::new(prompt, KdfParams::recommended())
    }

    pub fn theme() -> ColorfulTheme {
        ColorfulTheme {
            success_prefix: style("OK".to_owned()).for_stderr().green(),
            prompt_prefix: style("::".to_owned()).blue().for_stderr(),
            active_item_style: Style::new().for_stderr().magenta().bright(),
            active_item_prefix: style("->".to_owned()).magenta().bright().for_stderr(),
            picked_item_prefix: style("->".to_owned()).magenta().bright().for_stderr(),
            inactive_item_prefix: style("  ".to_string()).for_stderr(),

            ..ColorfulTheme::default()
        }
    }

    pub fn text_input(message: &str, default: Option<String>) -> String {
        let theme = theme();

        match default {
            Some(default) => Input::with_theme(&theme)
                .with_prompt(message)
                .default(default)
                .interact_text()
                .unwrap(),
            None => Input::with_theme(&theme)
                .with_prompt(message)
                .interact_text()
                .unwrap(),
        }
    }

    pub fn secret_input() -> SecUtf8 {
        SecUtf8::from(
            Password::with_theme(&theme())
                .with_prompt("Passphrase")
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

    pub fn select<'a, T>(options: &'a [T], active: &'a T) -> &'a T
    where
        T: fmt::Display + Eq + PartialEq,
    {
        let theme = theme();
        let active = options.iter().position(|o| o == active);
        let mut selection = dialoguer::Select::with_theme(&theme);

        if let Some(active) = active {
            selection.default(active);
        }
        let index = selection
            .items(&options.iter().map(|p| p.to_string()).collect::<Vec<_>>())
            .interact()
            .unwrap();

        &options[index]
    }

    pub mod format {
        use dialoguer::console::style;
        use librad::profile::Profile;

        use super::theme;

        pub fn highlight<D: std::fmt::Display>(input: D) -> String {
            style(input).green().bright().to_string()
        }

        pub fn error(header: &str, error: &anyhow::Error) {
            let err = style(error).red().to_string();
            let err = err.trim_end();
            let separator = if err.len() > 160 { "\n" } else { " " };

            eprintln!(
                "{} {}{}{}",
                style("==").red(),
                style(header).on_red(),
                separator,
                err
            );
        }

        pub fn profile_select<'a>(profiles: &'a [Profile], active: &Profile) -> &'a Profile {
            let active = profiles.iter().position(|p| p.id() == active.id()).unwrap();
            let selection = dialoguer::Select::with_theme(&theme())
                .items(&profiles.iter().map(|p| p.id()).collect::<Vec<_>>())
                .default(active)
                .interact()
                .unwrap();

            &profiles[selection]
        }
    }
}
