// Copyright © 2021 The Radicle Link Contributors
//
// This file is part of radicle-link, distributed under the GPLv3 with Radicle
// Linking Exception. For full terms see the included LICENSE file.

pub mod profile {

    use super::tui;
    use librad::{
        crypto::peer::PeerId,
        git::{storage::Storage, Urn},
        profile::{Profile, RadHome},
    };
    use rad_profile;
    use std::path::PathBuf;

    pub fn default() -> Option<Profile> {
        match rad_profile::get(None, None) {
            Ok(profile) => profile,
            Err(err) => {
                tui::error(&format!("Could not get active profile. {:?}", err));
                None
            }
        }
    }

    pub fn repo(home: &RadHome, profile: &Profile) -> Option<PathBuf> {
        match home {
            RadHome::Root(buf) => {
                let mut path = buf.to_path_buf();
                path.push(profile.id());
                path.push("git");
                Some(path)
            }
            _ => None,
        }
    }

    pub fn user(storage: &Storage) -> Option<Urn> {
        match storage.config_readonly() {
            Ok(config) => match config.user() {
                Ok(urn) => urn,
                Err(err) => {
                    tui::error(&format!("Could not read user. {:?}", err));
                    None
                }
            },
            Err(_err) => None,
        }
    }

    pub fn peer_id(storage: &Storage) -> Option<PeerId> {
        match storage.config_readonly() {
            Ok(config) => match config.peer_id() {
                Ok(peer_id) => Some(peer_id),
                Err(err) => {
                    tui::error(&format!("Could not read peer id. {:?}", err));
                    None
                }
            },
            Err(_err) => None,
        }
    }
}

pub mod person {

    use super::tui;
    use librad::{
        canonical::Cstring,
        git::{identities::Person, storage::Storage, Urn},
        identities::payload,
        profile::Profile,
    };
    use rad_clib::{keys::ssh::SshAuthSock, storage::ssh};
    use rad_identities::{self, local, person};

    pub fn get(storage: &Storage, urn: &Urn) -> Option<Person> {
        match person::get(storage, urn) {
            Ok(person) => person,
            Err(_) => None,
        }
    }

    pub fn create(profile: &Profile, name: &str) -> Option<Person> {
        let (signer, storage) = ssh::storage(&profile, SshAuthSock::default()).ok()?;
        let paths = profile.paths().clone();
        let payload = payload::Person {
            name: Cstring::from(name),
        };
        match person::create::<payload::Person>(
            &storage,
            paths,
            signer,
            payload.clone(),
            vec![],
            vec![],
            person::Creation::New { path: None },
        ) {
            Ok(person) => Some(person),
            Err(err) => {
                tui::error(&format!("Could not create person. {:?}", err));
                None
            }
        }
    }

    pub fn set_local(storage: &Storage, person: &Person) -> Option<Person> {
        let urn = person.urn();
        match local::get(&storage, urn.clone()) {
            Ok(identity) => match identity {
                Some(ident) => match local::set(&storage, ident) {
                    Ok(_) => Some(person.clone()),
                    Err(err) => {
                        tui::error(&format!("Could not set local identity. {:?}", err));
                        None
                    }
                },
                None => None,
            },
            Err(err) => {
                tui::error(&format!("Could not read identity. {:?}", err));
                None
            }
        }
    }
}

pub mod project {

    use super::tui;
    use librad::{
        crypto::BoxedSigner,
        git::identities::Project,
        git::storage::Storage,
        identities::payload::{self},
        profile::Profile,
    };
    use rad_identities::{self, project};
    use std::path::{Path, PathBuf};

    pub fn create(
        storage: &Storage,
        signer: BoxedSigner,
        profile: &Profile,
        payload: payload::Project,
    ) -> Option<Project> {
        let path = Path::new("../").to_path_buf();
        let paths = profile.paths().clone();
        let whoami = project::WhoAmI::from(None);
        let delegations = Vec::new().into_iter().collect();
        match project::create::<payload::Project>(
            &storage,
            paths,
            signer,
            whoami,
            delegations,
            payload,
            vec![],
            rad_identities::project::Creation::Existing { path },
        ) {
            Ok(project) => Some(project),
            Err(err) => {
                tui::error("Project could not be initialized.");
                tui::format::error_detail(&format!("{}", err));
                None
            }
        }
    }

    pub fn list(storage: &Storage) -> Result<Vec<Project>, project::Error> {
        let list = project::list(&storage)?;
        let projects = list.collect::<Result<Vec<_>, _>>()?;
        Ok(projects)
    }

    pub fn current() -> Option<PathBuf> {
        let path = Path::new(".git");
        if path.exists() {
            Some(path.to_path_buf())
        } else {
            tui::error("This is not a git repository.");
            None
        }
    }
}

pub mod keys {

    use super::tui;
    use librad::{
        crypto::keystore::{
            crypto::Pwhash,
            pinentry::{Pinentry, SecUtf8},
        },
        git::storage::Storage,
        profile::{Profile, ProfileId},
    };
    use rad_clib::{keys::ssh::SshAuthSock, storage::ssh};

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

    pub fn storage(profile: &Profile, sock: SshAuthSock) -> Option<Storage> {
        match ssh::storage(&profile, sock) {
            Ok((_, storage)) => Some(storage),
            Err(err) => {
                tui::error("Could not read ssh key:");
                tui::format::error_detail(&format!("{}", err));
                println!();
                None
            }
        }
    }

    pub fn add(
        profile: &Profile,
        pass: Pwhash<CachedPrompt>,
        sock: SshAuthSock,
    ) -> Option<ProfileId> {
        match rad_profile::ssh_add(None, profile.id().clone(), sock, pass, &Vec::new()) {
            Ok(id) => Some(id),
            Err(err) => {
                tui::error(&format!("Could not add ssh key. {:?}", err));
                println!();
                None
            }
        }
    }
}

pub mod seed {
    use librad::crypto::peer::PeerId;
    use std::{
        path::PathBuf,
        process::{Command, Stdio},
    };
    pub fn push_delegate_id(repo: &PathBuf, seed: &str, self_id: &str, peer_id: PeerId) {
        Command::new("git")
            .stdout(Stdio::null())
            .current_dir(repo.as_path())
            .arg("push")
            .arg("-q")
            .arg("--signed")
            .arg(format!("{}/{}", seed, self_id))
            .arg(format!(
                "refs/namespaces/{}/refs/rad/id:refs/remotes/{}/rad/id",
                self_id, peer_id
            ))
            .spawn()
            .expect("Git failed to start.");
    }

    pub fn push_project_id(repo: &PathBuf, seed: &str, project_id: &str, peer_id: PeerId) {
        Command::new("git")
            .stdout(Stdio::null())
            .current_dir(repo.as_path())
            .arg("push")
            .arg("-q")
            .arg("--signed")
            .arg("--atomic")
            .arg(format!("{}/{}", seed, project_id))
            .arg(format!(
                "refs/namespaces/{}/refs/rad/id:refs/remotes/{}/rad/id",
                project_id, peer_id
            ))
            .spawn()
            .expect("Git failed to start.");
    }

    pub fn push_refs(repo: &PathBuf, seed: &str, project_id: &str, peer_id: PeerId) {
        Command::new("git")
            .stdout(Stdio::null())
            .current_dir(repo.as_path())
            .arg("push")
            .arg("-q")
            .arg("--signed")
            .arg("--atomic")
            .arg(format!("{}/{}", seed, project_id))
            .arg(format!(
                "refs/namespaces/{}/refs/rad/ids/*:refs/remotes/{}/rad/ids/*",
                project_id, peer_id
            ))
            .arg(format!(
                "refs/namespaces/{}/refs/rad/signed_refs:refs/remotes/{}/rad/signed_refs",
                project_id, peer_id
            ))
            .arg(format!(
                "+refs/namespaces/{}/refs/heads/*:refs/remotes/{}/heads/*",
                project_id, peer_id
            ))
            .spawn()
            .expect("Git failed to start.");
    }
}

pub mod tui {

    use super::keys;
    use librad::crypto::keystore::{
        crypto::{KdfParams, Pwhash},
        pinentry::SecUtf8,
    };

    use dialoguer::{console::style, theme::ColorfulTheme, Input, Password};
    use indicatif::ProgressBar;

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
                .default(default.into())
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

        use librad::{
            git::{identities::Project, Urn},
            profile::Profile,
        };

        use dialoguer::console::style;

        pub fn profile_list(profiles: &Vec<Profile>, active: &Profile) {
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

        pub fn project_list(projects: &Vec<Project>) {
            for p in projects {
                println!("  {} {}", style("⋅").white(), &p.urn().to_string());
            }
            println!("");
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

        pub fn error_detail(detail: &str) {
            println!("  {} {}", style("⊙").red(), &detail);
        }
    }
}

pub mod proc {
    pub fn some_or_exit<T>(option: Option<T>) -> T {
        match option {
            Some(value) => value,
            None => std::process::exit(0),
        }
    }
}
