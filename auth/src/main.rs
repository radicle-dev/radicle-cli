use rad_clib::keys::ssh::SshAuthSock;

use rad_common::{keys, person, profile};
use rad_terminal::compoments as term;

mod args;

fn main() -> anyhow::Result<()> {
    match run() {
        Ok(()) => Ok(()),
        Err(err) => {
            term::format::error("Authentication failed", &err);
            term::blank();

            std::process::exit(1);
        }
    }
}

fn run() -> anyhow::Result<()> {
    let args = args::parse()?;

    let sock = SshAuthSock::default();

    let profiles = rad_profile::list(None)?;
    if !profiles.is_empty() && !args.new {
        let profile = profile::default()?;

        term::info(&format!(
            "Your active profile is {}",
            term::format::highlight(&profile.id().to_string())
        ));

        let selection = term::format::profile_select(&profiles, &profile);

        if !keys::is_ready(selection, sock.clone())? {
            term::warning("Your profile key is not in ssh-agent");

            let pass = term::pwhash(term::secret_input());
            let spinner = term::spinner("Unlocking...");

            keys::add(selection, pass, sock)?;
            spinner.finish();

            term::success("Profile key added to ssh-agent");
        }

        if selection.id() != profile.id() {
            let id = selection.id();
            profile::set(id)?;

            term::success(&format!("Profile changed to {}", id));
        }
    } else {
        term::headline("Initializing your 🌱 profile and identity");

        let username = term::text_input("Username", None);
        let pass = term::pwhash(term::secret_input_with_confirmation());

        let mut spinner = term::spinner("Creating your 🌱 Ed25519 keypair...");
        let (profile, peer_id) = rad_profile::create(None, pass.clone())?;

        spinner.finish();
        spinner = term::spinner("Adding to ssh-agent...");

        let profile_id = keys::add(&profile, pass, sock.clone())?;
        let (_, storage) = keys::storage(&profile, sock)?;

        spinner.finish();

        let person = person::create(&profile, &username)?;
        person::set_local(&storage, &person);

        term::success(&format!(
            "Profile {} created.",
            term::format::highlight(&profile_id.to_string())
        ));

        term::blank();
        term::info(&format!(
            "Your radicle Peer ID is {}. This identifies your device.",
            term::format::highlight(&peer_id.to_string())
        ));
        term::info(&format!(
            "Your personal 🌱 URN is {}. This identifies you across devices.",
            term::format::highlight(&person.urn().to_string())
        ));
    }
    Ok(())
}
