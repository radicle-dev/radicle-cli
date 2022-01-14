use librad::{
    git::Urn,
    profile::RadHome
};

use rad_clib::{keys::ssh::SshAuthSock};

use libcli::{keys, tui, profile, project, seed};

fn main() -> anyhow::Result<()> {
    match run() {
        Ok(()) => Ok(()),
        Err(_) => std::process::exit(1),
    }
}

fn run() -> anyhow::Result<()> {
    tui::headline("Publishing your local 🌱 project");

    let seed = "http://localhost:8778".to_string();
    let home = RadHome::default();

    let profile = profile::default()?;
    let storage = keys::storage(&profile, SshAuthSock::default())?;

    let repo = project::repository()?;
    let remote = project::remote(&repo)?;
    let project_id = Urn::encode_id(&remote.url.urn);

    let peer_id = profile::peer_id(&storage)?;
    let urn = profile::user(&storage)?;
    let monorepo = profile::repo(&home, &profile)?;
    let self_id = Urn::encode_id(&urn);

    tui::info("Using config:");
    tui::format::seed_config(&seed, &profile, &urn);

    tui::info(&format!("Syncing project {:?}", project_id));    

    let mut spinner = tui::spinner("Pushing delegate id...");
    seed::push_delegate_id(&monorepo, &seed, &self_id, peer_id)?;
    
    spinner.finish();              
    spinner = tui::spinner("Pushing project id...");
    seed::push_project_id(&monorepo, &seed, &project_id, peer_id)?;
    
    spinner.finish();   
    spinner = tui::spinner("Pushing rad/*, signed refs and heads...");
    seed::push_refs(&monorepo, &seed, &project_id, peer_id)?;
    
    spinner.finish();
    tui::success("Projects published.");
    Ok(())
}
