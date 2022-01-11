use radicle_tools::cli;

use structopt::StructOpt;

use rad_clib::keys::ssh::SshAuthSock;

use cli::{keys, person, profile, tui};

#[derive(Debug, StructOpt)]
pub struct Args {
    #[structopt(short, long)]
    pub add: bool,
}

fn main() -> anyhow::Result<()> {
    match run() {
        Ok(()) => Ok(()),
        Err(_) => {
            println!();
            std::process::exit(0);
        }
    }
}

fn run() -> anyhow::Result<()> {
    let Args { add } = Args::from_args();
    let sock = SshAuthSock::default();

    tui::headline("Initializing your 🌱 profile and identity");

    let profiles = rad_profile::list(None)?;
    if profiles.len() > 0 && !add {
        tui::warning("Found existing profile(s):");
        let profile = profile::default()?;
        tui::format::profile_list(&profiles, &profile);
        tui::info("If you want to create a new profile, please use --add.");
    } else {
        let username = tui::text_input("Username", None);
        let pass = tui::pwhash(tui::secret_input());

        let mut spinner = tui::spinner("Creating your profile...");
        let (profile, _) = rad_profile::create(None, pass.clone())?;
        spinner.finish();
        spinner = tui::spinner("Adding your key to ssh-agent...");

        let _id = keys::add(&profile, pass, sock.clone())?;
        let storage = keys::storage(&profile, sock)?;

        spinner.finish();
        spinner = tui::spinner("Creating identity...");

        let person = person::create(&profile, &username)?;
        person::set_local(&storage, &person);
        spinner.finish();
        tui::success("Profile and identity created.");
    }
    Ok(())
}
