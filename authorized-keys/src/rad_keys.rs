use std::convert::TryFrom;
use std::fs::{create_dir_all, read_dir, remove_file, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use async_trait::async_trait;
use pgp::types::KeyTrait;
use pgp::Deserializable;

use crate::{
    error::{Error, TIMEOUT_STDIN_WARNING},
    Action, KeyRing, KeyType, Options, RadKeyring,
};

// Default directory for storing keys;
const DEFAULT_RAD_KEYS_DIRECTORY: &str = ".rad/keys/";

/// local repository keyring authority;
#[derive(Debug, Clone)]
pub struct RadKeys {
    /// `.rad/keys/` directory path;
    pub keys_dir: PathBuf,
}

impl TryFrom<PathBuf> for RadKeys {
    type Error = Error;
    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        if !path.exists() {
            return Err(Error::MissingKeysDirectory);
        }

        Ok(Self { keys_dir: path })
    }
}

impl RadKeys {
    pub fn new(keys_dir: Option<PathBuf>) -> Result<Self, Error> {
        let dir = match keys_dir {
            Some(d) => d,
            None => {
                let mut dir = std::env::current_dir()?;
                // Push to the default radicle keys directory;
                dir.push(DEFAULT_RAD_KEYS_DIRECTORY);
                dir
            }
        };

        if !dir.exists() {
            // create the directory if it does not exist;
            create_dir_all(dir.clone())?;
        }

        Ok(Self { keys_dir: dir })
    }

    /// used by the rad-acl CLI to apply changes to the authorized keys files;
    pub async fn apply_options(options: Options) -> Result<(), Error> {
        let mut keys = Self::new(options.dir.clone())?;

        // default to OpenPGP key type if no `--type` is found;
        let key_type = options.key_type.clone().unwrap_or(KeyType::OpenPgp);

        // update the authorized keys
        match options.action {
            Action::Add => {
                let (key_id, key) = RadKeys::key_details(&options).await?;

                RadKeys::add(&mut keys, key_type, key_id, key.as_slice()).await?;
            }
            Action::Remove => {
                if let Some(id) = options.id {
                    RadKeys::remove(&mut keys, key_type, id).await?;
                } else {
                    return Err(Error::MissingKeyId);
                }
            }
            Action::List => {
                let keys = RadKeys::keyring(&keys, key_type).await?.join(",");

                println!("\tRadicle Authorized Keys: {}", keys);
            }
        };

        Ok(())
    }

    pub async fn key_details(options: &Options) -> Result<(String, Vec<u8>), Error> {
        let key_type = options.key_type.clone().unwrap_or(KeyType::OpenPgp);

        Ok(match key_type {
            KeyType::OpenPgp => {
                // check ID of
                let (pk, headers) = if let Some(path) = &options.path {
                    let file = std::fs::File::open(path)?;
                    pgp::SignedPublicKey::from_armor_single(file)?
                } else {
                    let (rx, tx) = mpsc::channel::<String>();

                    // wait for standard input, otherwise timeout after 3000 ms;
                    thread::spawn(move || {
                        let mut buf = String::new();
                        std::io::stdin()
                            .read_to_string(&mut buf)
                            .map_err(|e| mpsc::SendError(e.to_string()))?;

                        rx.send(buf)
                    });

                    let src = match tx.recv_timeout(Duration::from_millis(3000)) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("{}", TIMEOUT_STDIN_WARNING);
                            return Err(Error::TimeoutError(e));
                        }
                    };

                    pgp::SignedPublicKey::from_string(&src)?
                };

                // verify the key is valid:
                pk.verify()?;

                // extract the primary key id
                let key_id = hex::encode(pk.primary_key.key_id().as_ref()).to_uppercase();
                let key = pk.to_armored_bytes(Some(&headers))?;
                (key_id, key)
            }
            // TODO: Support Ethereum and Ed25519 keys;
            _ => return Err(Error::UnsupportedKey),
        })
    }

    /// set the `.rad/keys` directory;
    pub fn set_keys_dir(&mut self, dir: PathBuf) -> Result<(), Error> {
        self.keys_dir = dir;
        Ok(())
    }

    /// return the path to the key file;
    pub fn key_path(&self, key_type: KeyType, key_id: &str) -> Result<PathBuf, Error> {
        let mut path = self.keys_dir.clone();

        // push the key type;
        path.push(&key_type.to_string());

        if !path.exists() {
            // create the directory if it does not exist;
            create_dir_all(path.clone())?;
        }

        // push the key id file that will contain the full public key;
        path.push(&key_id);

        Ok(path)
    }
}

#[async_trait]
impl RadKeyring for RadKeys {
    type Error = Error;
    async fn add(
        &mut self,
        key_type: KeyType,
        key_id: String,
        pub_key: &[u8],
    ) -> Result<(), Self::Error> {
        let path = self.key_path(key_type, &key_id)?;

        // if the key already exists, ignore;
        if path.exists() {
            return Err(Error::KeyExists);
        }

        let mut file = File::create(&path)?;

        file.write_all(pub_key)?;

        println!("added .rad keys file to: {:?}", path);

        Ok(())
    }

    async fn remove(&mut self, key_type: KeyType, key_id: String) -> Result<(), Self::Error> {
        let path = self.key_path(key_type, &key_id)?;

        if !path.exists() {
            return Err(Error::KeyDoesNotExist);
        }
        remove_file(&path)?;

        println!("removed .rad keys file: {:?}", path);

        Ok(())
    }

    async fn keyring(&self, key_type: KeyType) -> Result<KeyRing, Self::Error> {
        let mut path = self.keys_dir.clone();

        // push the key type;
        path.push(&key_type.to_string());

        if !path.exists() {
            return Ok(Vec::new());
        }

        let keys = read_dir(path)?
            .into_iter()
            .filter_map(|f| f.ok())
            .filter_map(|f| f.file_name().into_string().ok())
            .collect::<Vec<String>>();

        Ok(keys)
    }

    async fn exists(&self, key_type: KeyType, key_id: String) -> Result<bool, Self::Error> {
        let path = self.key_path(key_type, &key_id)?;

        // check the contents of the file
        let file = std::fs::File::open(path)?;
        let (pk, _) = pgp::SignedPublicKey::from_armor_single(file)?;

        // verify key is valid;
        pk.verify()?;

        // extract fingerprint key id from public key and ensure it matches the key_id;
        let fingerprint = hex::encode(pk.primary_key.key_id().as_ref()).to_uppercase();

        // return whether the fingerprint matches the key id in question;
        Ok(fingerprint == key_id)
    }
}
