extern crate rad_cli;

use librad::profile::RadHome;

use rad_clib::{keys::ssh::SshAuthSock};

use rad_cli::{proc::some_or_exit, keys, id, tui, profile, project, seed};

fn main() -> anyhow::Result<()> {
    tui::headline("Syncing your local 🌱 projects");

    let seed = "http://localhost:8778".to_string();
    let home = RadHome::default();

    let profile = some_or_exit(profile::default());
    let storage = some_or_exit(keys::storage(&profile, SshAuthSock::default()));
    

    let peer_id = some_or_exit(profile::peer_id(&storage));
    let urn = some_or_exit(profile::user(&storage));
    let monorepo = some_or_exit(profile::repo(&home, &profile));

    tui::info("Using config:");
    tui::format::seed_config(&seed, &profile, &urn);

    let projects = project::list(&storage)?;
    if projects.len() > 0 {
        let self_id = id::from_urn(&urn);

        for project in projects {
            tui::info(&format!("Syncing project {:?}", project.urn().to_string()));
            let project_id = id::from_urn(&project.urn());

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
    println!();
    Ok(())
}
