use std::path::PathBuf;

use anyhow::Result;
use assay::assay;

mod proposal {
    use super::*;
    use rad_common::test;
    use test::setup::*;

    fn create_auth_options(name: &str) -> rad_auth::Options {
        rad_auth::Options {
            active: false,
            init: true,
            username: Some(name.to_owned()),
            password: Some(test::USER_PASS.to_owned()),
        }
    }

    fn create_init_options(path: PathBuf, name: &str) -> rad_init::Options {
        rad_init::Options {
            path: Some(path),
            name: Some(name.to_owned()),
            description: Some("".to_owned()),
            branch: Some("master".to_owned()),
        }
    }

    #[assay(
        setup = with_steps(vec![
            Steps::CreateLnkHome,
            Steps::CreateGitRepo,
            Steps::InitGitRepo,
            Steps::CreateCommits
        ])?,
        teardown = test::teardown::profiles()?,
    )]
    fn can_be_stored() {
        use rad_common::proposal;
        let proposal: proposal::Metadata = Default::default();

        let auth_opts = create_auth_options("user");
        rad_auth::init(auth_opts)?;

        let init_opts = create_init_options(repo_path(), "project");
        rad_init::init(init_opts)?;

        let (storage, urn, repo) = environment()?;
        let result = proposal::store(&storage, &repo, &urn, &proposal, false);

        assert!(result.is_ok());
    }
}
