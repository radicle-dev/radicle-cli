pub mod args;

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

pub mod components {
    use std::fmt;
    use std::str::FromStr;

    use librad::crypto::keystore::crypto::{KdfParams, Pwhash};
    use librad::crypto::keystore::pinentry::SecUtf8;

    use dialoguer::{console::style, console::Style, theme::ColorfulTheme, Input, Password};
    use indicatif::{ProgressBar, ProgressFinish, ProgressStyle};

    use super::keys;

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
            $crate::components::success_args(format_args!($($arg)*));
        })
    }

    pub fn success_args(args: fmt::Arguments) {
        println!("{} {}", style("ok").green().reverse(), args);
    }

    pub use info;
    pub use success;

    #[derive(Debug)]
    pub struct Table<const W: usize> {
        rows: Vec<[String; W]>,
        widths: [usize; W],
    }

    impl<const W: usize> Table<W> {
        pub fn default() -> Self {
            Self {
                rows: Vec::new(),
                widths: [0; W],
            }
        }

        pub fn push(&mut self, row: [String; W]) {
            for (i, cell) in row.iter().enumerate() {
                self.widths[i] = self.widths[i].max(console::measure_text_width(cell));
            }
            self.rows.push(row);
        }

        pub fn render(self) {
            for row in &self.rows {
                for (i, cell) in row.iter().enumerate() {
                    print!(
                        "{} ",
                        console::pad_str(cell, self.widths[i], console::Alignment::Left, None)
                    );
                }
                println!();
            }
        }

        pub fn render_tree(self) {
            for (r, row) in self.rows.iter().enumerate() {
                if r != self.rows.len() - 1 {
                    print!("├── ");
                } else {
                    print!("└── ");
                }
                for (i, cell) in row.iter().enumerate() {
                    print!(
                        "{} ",
                        console::pad_str(cell, self.widths[i], console::Alignment::Left, None)
                    );
                }
                println!();
            }
        }
    }

    pub struct Spinner {
        progress: ProgressBar,
        message: String,
    }

    impl Spinner {
        pub fn finish(self) {
            self.progress.finish_and_clear();
            self::success!("{}", &self.message);
        }

        pub fn failed(self) {
            self.progress.finish_and_clear();
            self::eprintln(style("!!").red().reverse(), self.message);
        }

        pub fn clear(self) {
            self.progress.finish_and_clear();
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
            .map(|line| format!("{}{}\n", prefix, line))
            .collect()
    }

    pub fn help(name: &str, version: &str, description: &str, usage: &str) {
        println!("rad-{} v{}\n{}\n{}", name, version, description, usage);
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

    pub fn tip(msg: &str) {
        println!("{} {}", style("=>").blue(), style(msg).dim().italic());
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

    pub fn failure(bin: &str, error: &anyhow::Error) {
        eprintln!(
            "{} {} {} {}",
            style("==").red(),
            style("Error:").red(),
            style(format!("rad-{}:", bin)).red(),
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

    pub fn confirm<D: fmt::Display>(prompt: D) -> bool {
        dialoguer::Confirm::new()
            .with_prompt(format!("{} {}", style(" ⤷".to_owned()).cyan(), prompt))
            .wait_for_newline(true)
            .default(false)
            .interact()
            .unwrap_or_default()
    }

    pub fn pwhash(secret: SecUtf8) -> Pwhash<keys::CachedPrompt> {
        let prompt = keys::CachedPrompt::new(secret);
        Pwhash::new(prompt, KdfParams::recommended())
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

    fn _info(args: std::fmt::Arguments) {
        println!("{}", args);
    }

    pub mod format {
        use dialoguer::console::style;
        use librad::profile::Profile;

        use super::theme;

        pub fn secondary<D: std::fmt::Display>(msg: D) -> String {
            style(msg).blue().to_string()
        }

        pub fn tertiary<D: std::fmt::Display>(msg: D) -> String {
            style(msg).cyan().to_string()
        }

        pub fn yellow<D: std::fmt::Display>(msg: D) -> String {
            style(msg).yellow().to_string()
        }

        pub fn highlight<D: std::fmt::Display>(input: D) -> String {
            style(input).green().bright().to_string()
        }

        pub fn bold<D: std::fmt::Display>(input: D) -> String {
            style(input).bold().to_string()
        }

        pub fn dim<D: std::fmt::Display>(input: D) -> String {
            style(input).dim().to_string()
        }

        pub fn italic<D: std::fmt::Display>(input: D) -> String {
            style(input).italic().dim().to_string()
        }

        pub fn error(header: &str, error: &anyhow::Error) {
            use crate::args::Error;

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
                super::blank();
            }

            if let Some(Error::WithHint { hint, .. }) = error.downcast_ref::<Error>() {
                eprintln!("{}", &style(hint).yellow().to_string());
                super::blank();
            }
        }

        pub fn profile_select<'a>(
            profiles: &'a [Profile],
            active: &Profile,
        ) -> Option<&'a Profile> {
            let active = profiles.iter().position(|p| p.id() == active.id()).unwrap();
            let selection = dialoguer::Select::with_theme(&theme())
                .items(&profiles.iter().map(|p| p.id()).collect::<Vec<_>>())
                .default(active)
                .interact_opt()
                .unwrap();

            selection.map(|i| &profiles[i])
        }
    }
}
