//! Person-related functionality.
use anyhow::{Error, Result};

use librad::{canonical::Cstring, git::identities::local::LocalIdentity};

use librad::git::identities::Person;
use librad::git::storage::Storage;

use librad::crypto::BoxedSigner;
use librad::identities::payload;
use librad::identities::payload::HasNamespace;
use librad::profile::Profile;

use rad_identities::{self, local, person};
use rad_terminal::components as term;

pub use librad::git::identities::person::verify;
pub use person::get;

lazy_static::lazy_static! {
    static ref ENS_NAMESPACE: url::Url = "https://radicle.xyz/ethereum/ens/v1"
        .parse()
        .expect("static URL malformed");
}

/// ENS payload.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Ens {
    pub name: String,
}

impl HasNamespace for Ens {
    fn namespace() -> &'static url::Url {
        &ENS_NAMESPACE
    }
}

/// Create a personal identity.
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

/// Set the local identity to the given person.
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

/// Get the current local identity.
pub fn local(storage: &Storage) -> Result<LocalIdentity, local::Error> {
    local::default(storage)
}

/// Set an ENS payload for the local identity.
/// Returns the updated person.
pub fn set_ens_payload(ens: Ens, storage: &Storage) -> Result<Person> {
    let id = local::default(storage)?;
    let payload = id.payload();
    let mut exts = payload
        .exts()
        .map(|(namespace, val)| (namespace.clone(), val.clone()))
        .map(|(namespace, val)| payload::Ext { namespace, val })
        .collect::<Vec<_>>();

    let namespace = Ens::namespace().clone();
    let val = serde_json::to_value(ens)?;
    let delegations = id.delegations().iter().cloned();

    exts.push(payload::Ext { namespace, val });

    let new = person::update(
        storage,
        &id.urn(),
        Some(id.urn()),
        None,
        exts,
        Some(delegations),
    )?;

    Ok(new)
}
