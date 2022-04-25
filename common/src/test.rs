use std::{env, error};

use librad::crypto::keystore::crypto::{Pwhash, KDF_PARAMS_TEST};
use librad::crypto::keystore::pinentry::SecUtf8;
use librad::profile::LNK_HOME;

use super::{keys, profile, test};

pub type BoxedError = Box<dyn error::Error>;

pub const USER_PASS: &str = "password";

pub mod setup {
    use super::*;
    pub fn lnk_home() -> Result<(), BoxedError> {
        env::set_var(LNK_HOME, env::current_dir()?.join("lnk_home"));
        Ok(())
    }
}

pub mod teardown {
    use super::*;
    pub fn profiles() -> Result<(), BoxedError> {
        if let Ok(profiles) = profile::list() {
            for profile in profiles {
                let pass = Pwhash::new(SecUtf8::from(test::USER_PASS), *KDF_PARAMS_TEST);
                keys::remove(&profile, pass, keys::ssh_auth_sock())?;
            }
        }
        Ok(())
    }
}
