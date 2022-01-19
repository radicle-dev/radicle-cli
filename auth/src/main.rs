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
            spinner.finish_and_clear();

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

        let mut spinner = term::spinner("Creating your profile...");
        let (profile, _) = rad_profile::create(None, pass.clone())?;
        spinner.finish();
        spinner = term::spinner("Adding your key to ssh-agent...");

        let _id = keys::add(&profile, pass, sock.clone())?;
        let (_, storage) = keys::storage(&profile, sock)?;

        spinner.finish();
        spinner = term::spinner("Creating identity...");

        let person = person::create(&profile, &username)?;
        person::set_local(&storage, &person);
        spinner.finish();
        term::success("Profile and identity created.");
    }
    Ok(())
}
