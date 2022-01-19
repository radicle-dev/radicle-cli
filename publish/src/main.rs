use librad::git::Urn;
use librad::profile::RadHome;

use rad_clib::keys::ssh::SshAuthSock;
use rad_common::{keys, profile, project, seed};
use rad_terminal::compoments as term;

fn main() -> anyhow::Result<()> {
    match run() {
        Ok(()) => Ok(()),
        Err(_) => std::process::exit(1),
    }
}

fn run() -> anyhow::Result<()> {
    term::headline("Publishing your local 🌱 project");

    let seed = "http://localhost:8778".to_string();
    let home = RadHome::default();

    let profile = profile::default()?;
    let (_, storage) = keys::storage(&profile, SshAuthSock::default())?;

    let repo = project::repository()?;
    let remote = project::remote(&repo)?;
    let project_id = Urn::encode_id(&remote.url.urn);

    let peer_id = profile::peer_id(&storage)?;
    let urn = profile::user(&storage)?;
    let monorepo = profile::repo(&home, &profile)?;
    let self_id = Urn::encode_id(&urn);

    term::info("Using config:");
    term::format::seed_config(&seed, &profile, &urn);

    term::info(&format!("Syncing project {:?}", project_id));

    let mut spinner = term::spinner("Pushing delegate id...");
    seed::push_delegate_id(&monorepo, &seed, &self_id, peer_id)?;

    spinner.finish();
    spinner = term::spinner("Pushing project id...");
    seed::push_project_id(&monorepo, &seed, &project_id, peer_id)?;

    spinner.finish();
    spinner = term::spinner("Pushing rad/*, signed refs and heads...");
    seed::push_refs(&monorepo, &seed, &project_id, peer_id)?;

    spinner.finish();
    term::success("Projects published.");
    Ok(())
}
