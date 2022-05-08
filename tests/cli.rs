use std::process::Command;

use anyhow::Result;
use assay::assay;
use assert_cmd::prelude::*;

use radicle_common::test;

mod auth {
    use super::*;

    const NAME_MISSING: &str = "missing argument for option '--name'";
    const PASSPHRASE_MISSING: &str = "missing argument for option '--passphrase'";
    const INIT_MISSING: &str = "invalid option '--passphrase'";

    #[assay(
        setup = test::setup::lnk_home()?,
        teardown = test::teardown::profiles()?,
    )]
    fn can_be_initialized() {
        let status = Command::cargo_bin("rad-auth")?
            .args(["--init", "--name", "user1", "--passphrase", test::USER_PASS])
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
    fn passphrase_missing() {
        let output = Command::cargo_bin("rad-auth")?
            .args(["--init", "--passphrase"])
            .output()?;
        let result = String::from_utf8_lossy(&output.stderr);

        assert!(result.contains(PASSPHRASE_MISSING), "{}", result);
    }

    #[assay]
    fn init_missing() {
        let output = Command::cargo_bin("rad-auth")?
            .args(["--passphrase"])
            .output()?;
        let result = String::from_utf8_lossy(&output.stderr);

        assert!(result.contains(INIT_MISSING), "{}", result);
    }
}
