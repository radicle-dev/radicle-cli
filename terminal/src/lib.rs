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
    use std::fmt;

    use librad::crypto::keystore::crypto::{KdfParams, Pwhash};
    use librad::crypto::keystore::pinentry::SecUtf8;

    use dialoguer::{console::style, theme::ColorfulTheme, Input, Password};
    use indicatif::{ProgressBar, ProgressFinish, ProgressStyle};

    use super::keys;

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
            self::eprintln(style("✗").red(), self.message);
        }
    }

    pub fn headline(headline: &str) {
        println!();
        println!("{}", style(headline).bold());
        println!();
    }

    pub fn blank() {
        println!()
    }

    pub fn eprintln(prefix: impl fmt::Display, msg: impl fmt::Display) {
        eprintln!("{} {}", prefix, msg);
    }

    pub fn info(info: &str) {
        println!("{} {}", style("ℹ").blue(), info);
    }

    pub fn warning(warning: &str) {
        eprintln!("{} {}", style("⚠").yellow(), warning);
    }

    pub fn error(error: &str) {
        eprintln!("{} {}", style("✗").red(), style(error).red());
    }

    pub fn success(success: &str) {
        println!("{} {}", style("✔").green(), success);
    }

    pub fn spinner(message: &str) -> Spinner {
        let message = message.to_owned();
        let style = ProgressStyle::default_spinner()
            .tick_strings(&[
                &style("⠁").yellow().to_string(),
                &style("⠈").yellow().to_string(),
                &style("⠐").yellow().to_string(),
                &style("⠠").yellow().to_string(),
                &style("⠄").yellow().to_string(),
                &style("⠂").yellow().to_string(),
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

    pub fn text_input(message: &str, default: Option<String>) -> String {
        match default {
            Some(default) => Input::with_theme(&ColorfulTheme::default())
                .with_prompt(message)
                .default(default)
                .interact_text()
                .unwrap(),
            None => Input::with_theme(&ColorfulTheme::default())
                .with_prompt(message)
                .interact_text()
                .unwrap(),
        }
    }

    pub fn secret_input() -> SecUtf8 {
        SecUtf8::from(
            Password::with_theme(&ColorfulTheme::default())
                .with_prompt("Passphrase")
                .interact()
                .unwrap(),
        )
    }

    pub fn secret_input_with_confirmation() -> SecUtf8 {
        SecUtf8::from(
            Password::with_theme(&ColorfulTheme::default())
                .with_prompt("Passphrase")
                .with_confirmation("Repeat passphrase", "Error: the passphrases don't match.")
                .interact()
                .unwrap(),
        )
    }

    pub mod format {
        use dialoguer::console::style;
        use dialoguer::theme::ColorfulTheme;
        use librad::git::Urn;
        use librad::profile::Profile;

        pub fn highlight<D: std::fmt::Display>(input: D) -> String {
            style(input).green().bold().to_string()
        }

        pub fn error(header: &str, error: &anyhow::Error) {
            eprintln!(
                "{} {} {:#}",
                style("✗").red(),
                style(header).on_red(),
                style(error).red()
            );
        }

        pub fn error_detail(detail: &str) {
            eprintln!("  {} {}", style("⊙").red(), &detail);
        }

        pub fn profile_select<'a>(profiles: &'a [Profile], active: &Profile) -> &'a Profile {
            let active = profiles.iter().position(|p| p.id() == active.id()).unwrap();
            let selection = dialoguer::Select::with_theme(&ColorfulTheme::default())
                .items(&profiles.iter().map(|p| p.id()).collect::<Vec<_>>())
                .default(active)
                .interact()
                .unwrap();

            &profiles[selection]
        }

        pub fn seed_config(seed: &str, profile: &Profile, urn: &Urn) {
            println!("  ⋅ {} {}", style("(Seed)").magenta(), seed);
            println!(
                "  ⋅ {} {}",
                style("(Profile)").magenta(),
                &profile.id().to_string()
            );
            println!("  ⋅ {} {}", style("(Identity)").magenta(), &urn.to_string());
            println!();
        }
    }
}
