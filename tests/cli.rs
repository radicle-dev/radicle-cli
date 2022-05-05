use std::process::Command;

use anyhow::Result;
use assay::assay;
use assert_cmd::prelude::*;

use rad_common::test;

mod auth {
    use super::*;
    use test::setup::Env;
    use test::{setup, teardown};

    const NAME_MISSING: &str = "missing argument for option '--name'";
    const PASSWORD_MISSING: &str = "missing argument for option '--password'";
    const INIT_MISSING: &str = "invalid option '--password'";

    #[assay(
        setup = setup::with(&[Env::Home, Env::SshAgent])?,
        teardown = teardown::all()?,
    )]
    fn can_be_initialized() {
        let status = Command::cargo_bin("rad-auth")?
            .args(["--init", "--name", "user1", "--password", test::USER_PASS])
            .status();
        assert!(status?.success());
    }

    #[assay]
    fn username_missing() {
        let output = Command::cargo_bin("rad-auth")?
            .args(["--init", "--name"])
            .output()?;
        let result = String::from_utf8_lossy(&output.stderr);

        assert!(result.contains(NAME_MISSING), "{}", result);
    }

    #[assay]
    fn password_missing() {
        let output = Command::cargo_bin("rad-auth")?
            .args(["--init", "--password"])
            .output()?;
        let result = String::from_utf8_lossy(&output.stderr);

        assert!(result.contains(PASSWORD_MISSING), "{}", result);
    }

    #[assay]
    fn init_missing() {
        let output = Command::cargo_bin("rad-auth")?
            .args(["--password"])
            .output()?;
        let result = String::from_utf8_lossy(&output.stderr);

        assert!(result.contains(INIT_MISSING), "{}", result);
    }
}
