use anyhow::{Error, Result};

use librad::canonical::Cstring;

use librad::git::identities::Person;
use librad::git::storage::Storage;
use librad::git::Urn;

use librad::crypto::BoxedSigner;
use librad::identities::payload;
use librad::profile::Profile;

use rad_identities::{self, local, person};
use rad_terminal::components as term;

pub fn get(storage: &Storage, urn: &Urn) -> Option<Person> {
    match person::get(storage, urn) {
        Ok(person) => person,
        Err(_) => None,
    }
}

pub fn create(
    profile: &Profile,
    name: &str,
    signer: BoxedSigner,
    storage: &Storage,
) -> Result<Person, Error> {
    let paths = profile.paths().clone();
    let payload = payload::Person {
        name: Cstring::from(name),
    };
    match person::create::<payload::Person>(
        storage,
        paths,
        signer,
        payload,
        vec![],
        vec![],
        person::Creation::New { path: None },
    ) {
        Ok(person) => Ok(person),
        Err(err) => {
            term::error(&format!("Could not create person. {:?}", err));
            Err(err)
        }
    }
}

pub fn set_local(storage: &Storage, person: &Person) -> Option<Person> {
    let urn = person.urn();
    match local::get(storage, urn) {
        Ok(identity) => match identity {
            Some(ident) => match local::set(storage, ident) {
                Ok(_) => Some(person.clone()),
                Err(err) => {
                    term::error(&format!("Could not set local identity. {:?}", err));
                    None
                }
            },
            None => None,
        },
        Err(err) => {
            term::error(&format!("Could not read identity. {:?}", err));
            None
        }
    }
}
