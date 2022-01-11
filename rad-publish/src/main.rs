use radicle_tools::cli;

use librad::{
    git::Urn,
    profile::RadHome
};

use rad_clib::{keys::ssh::SshAuthSock};

use cli::{keys, tui, profile, project, seed};

fn main() -> anyhow::Result<()> {
    match run() {
        Ok(()) => Ok(()),
        Err(_) => std::process::exit(0),
    }
}

fn run() -> anyhow::Result<()> {
    tui::headline("Publishing your local 🌱 project");

    let seed = "http://localhost:8778".to_string();
    let home = RadHome::default();

    let profile = profile::default()?;
    let storage = keys::storage(&profile, SshAuthSock::default())?;
    

    let peer_id = profile::peer_id(&storage)?;
    let urn = profile::user(&storage)?;
    let monorepo = profile::repo(&home, &profile)?;

    tui::info("Using config:");
    tui::format::seed_config(&seed, &profile, &urn);

    let projects = project::list(&storage)?;
    if projects.len() > 0 {
        let self_id = Urn::encode_id(&urn);

        for project in projects {
            tui::info(&format!("Syncing project {:?}", project.urn().to_string()));
            let project_id = Urn::encode_id(&project.urn());

            let mut spinner = tui::spinner("Pushing delegate id...");
            seed::push_delegate_id(&monorepo, &seed, &self_id, peer_id);
            
            spinner.finish();              
            spinner = tui::spinner("Pushing project id...");
            seed::push_project_id(&monorepo, &seed, &project_id, peer_id);
            
            spinner.finish();   
            spinner = tui::spinner("Pushing rad/*, signed refs and heads...");
            seed::push_refs(&monorepo, &seed, &project_id, peer_id);
            
            spinner.finish();
        }
        tui::success("All projects synched.");
    } else {
        tui::warning("No exisiting project(s) found. Skipping sync.");
    }
    Ok(())
}
