use std::{env, error};

use librad::crypto::keystore::crypto;
use librad::crypto::keystore::pinentry::SecUtf8;
use librad::profile::LNK_HOME;

use super::{keys, profile, test};

pub type BoxedError = Box<dyn error::Error>;

pub const USER_PASS: &str = "password";

pub mod setup {
    use super::*;

    #[derive(PartialEq)]
    pub enum Env {
        Home,
    }

    pub fn with(environment: &[Env]) -> Result<(), BoxedError> {
        if environment.contains(&Env::Home) {
            env::set_var(LNK_HOME, env::current_dir()?.join("lnk_home"));
        }
        Ok(())
    }
}

pub mod teardown {
    use super::*;
    pub fn profiles() -> Result<(), BoxedError> {
        #[cfg(test)]
        let params = *crypto::KDF_PARAMS_TEST;
        #[cfg(not(test))]
        let params = crypto::KdfParams::recommended();

        if let Ok(profiles) = profile::list() {
            for profile in profiles {
                let pass = crypto::Pwhash::new(SecUtf8::from(test::USER_PASS), params);
                keys::remove(&profile, pass, keys::ssh_auth_sock())?;
            }
        }
        Ok(())
    }
}
