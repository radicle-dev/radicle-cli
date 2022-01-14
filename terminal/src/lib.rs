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
    use librad::crypto::keystore::crypto::{KdfParams, Pwhash};
    use librad::crypto::keystore::pinentry::SecUtf8;

    use dialoguer::{console::style, theme::ColorfulTheme, Input, Password};
    use indicatif::ProgressBar;

    use super::keys;

    pub fn headline(headline: &str) {
        println!();
        println!("{}", style(headline).bold());
        println!();
    }

    pub fn info(info: &str) {
        println!("{} {}", style("ℹ").blue(), info);
    }

    pub fn warning(warning: &str) {
        println!("{} {}", style("⚠").yellow(), warning);
    }

    pub fn error(error: &str) {
        println!("{} {}", style("✖").red(), error);
    }

    pub fn success(success: &str) {
        println!();
        println!("{} {}", style("✔").green(), success);
    }

    pub fn spinner(message: &str) -> ProgressBar {
        let spinner = ProgressBar::new_spinner();
        spinner.enable_steady_tick(120);
        spinner.set_message(message.to_string());
        spinner
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
                .with_prompt("Password")
                .with_confirmation("Repeat password", "Error: the passwords don't match.")
                .interact()
                .unwrap(),
        )
    }

    pub mod format {
        use dialoguer::console::style;
        use librad::git::Urn;
        use librad::profile::Profile;

        pub fn error_detail(detail: &str) {
            println!("  {} {}", style("⊙").red(), &detail);
        }

        pub fn profile_list(profiles: &[Profile], active: &Profile) {
            for p in profiles {
                if p.id() == active.id() {
                    println!(
                        "  {} {} {}",
                        style("⊙").magenta(),
                        &p.id().to_string(),
                        style("(active)").magenta()
                    );
                } else {
                    println!("  {} {}", style("⋅").white(), &p.id().to_string());
                }
            }
            println!();
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
