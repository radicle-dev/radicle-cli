//! SSH and key-related functions.
use anyhow::{Context as _, Error, Result};

use zeroize::Zeroizing;

use self::link_crypto::PublicKey;
use self::radicle_keystore as crypto;
use self::radicle_keystore::Pwhash;
use self::radicle_keystore::{FileStorage, Keystore};
use self::radicle_keystore::{Pinentry, SecUtf8};
use self::ssh_super::SshAuthSock;

use librad::PeerId;

use radicle::profile::Profile;
use radicle::Storage;

pub use lnk_clib::keys::LIBRAD_KEY_FILE as KEY_FILE;

use crate::signer::{ToSigner, ZeroizingSecretKey};

/// Env var used to pass down the passphrase to the git-remote-helper when
/// ssh-agent isn't present.
pub const RAD_PASSPHRASE: &str = "RAD_PASSPHRASE";

/// Get the radicle signer and storage.
pub fn storage(profile: &Profile, _signer: impl ToSigner) -> Result<Storage, Error> {
    let storage = Storage::open(profile.paths().storage())?;

    Ok(storage)
}

/// Add a profile's radicle signing key to ssh-agent.
pub fn add<P: Pinentry>(profile: &Profile, pass: Pwhash<P>, sock: SshAuthSock) -> Result<(), Error>
where
    <P as Pinentry>::Error: std::fmt::Debug + std::error::Error + Send + Sync + 'static,
{
    self::ssh::add_signer(profile, sock, pass, vec![]).context("could not add ssh key")?;

    Ok(())
}

/// Remove a profile's radicle signing key from the ssh-agent
pub fn remove<P: Pinentry>(
    profile: &Profile,
    pass: Pwhash<P>,
    sock: SshAuthSock,
) -> Result<(), Error>
where
    <P as Pinentry>::Error: std::fmt::Debug + std::error::Error + Send + Sync + 'static,
{
    self::ssh::remove_signer(profile, sock, pass).context("could not remove ssh key")?;

    Ok(())
}

/// Get the SSH auth socket and error if ssh-agent is not running.
pub fn ssh_auth_sock() -> Result<SshAuthSock, anyhow::Error> {
    if std::env::var("SSH_AGENT_PID").is_err() && std::env::var("SSH_AUTH_SOCK").is_err() {
        anyhow::bail!("ssh-agent does not appear to be running");
    }
    Ok(SshAuthSock::Env)
}

/// Check whether the radicle signing key has been added to ssh-agent.
pub fn is_ready(profile: &Profile, sock: SshAuthSock) -> Result<bool, Error> {
    self::ssh::is_signer_present(profile, sock)
        .context("could not lookup ssh key, is ssh-agent running?");
    Ok(true)
}

/// Get the SSH long key from a peer id.
/// This is the output of `ssh-add -L`.
pub fn to_ssh_key(peer_id: &PeerId) -> Result<String, std::io::Error> {
    use byteorder::{BigEndian, WriteBytesExt};

    let mut buf = Vec::new();
    let key = peer_id.as_public_key().as_ref();
    let len = key.len();

    buf.write_u32::<BigEndian>(len as u32)?;
    buf.extend_from_slice(key);

    // Despite research, I have no idea what this string is, but it seems
    // to be the same for all Ed25519 keys.
    let mut encoded = String::from("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5");
    encoded.push_str(&base64::encode(buf));

    Ok(encoded)
}

/// Get the SSH key fingerprint from a peer id.
/// This is the output of `ssh-add -l`.
pub fn to_ssh_fingerprint(peer_id: &PeerId) -> Result<String, std::io::Error> {
    use byteorder::{BigEndian, WriteBytesExt};
    use sha2::Digest;

    let mut buf = Vec::new();
    let name = b"ssh-ed25519";
    let key = peer_id.as_public_key().as_ref();

    buf.write_u32::<BigEndian>(name.len() as u32)?;
    buf.extend_from_slice(name);
    buf.write_u32::<BigEndian>(key.len() as u32)?;
    buf.extend_from_slice(key);

    let sha = sha2::Sha256::digest(&buf).to_vec();
    let encoded = base64::encode(sha);

    Ok(format!("SHA256:{}", encoded.trim_end_matches('=')))
}

/// Get a profile's secret key by providing a passphrase.
pub fn load_secret_key(
    profile: &Profile,
    passphrase: SecUtf8,
) -> Result<ZeroizingSecretKey, anyhow::Error> {
    let pwhash = pwhash(passphrase);
    let file_storage: FileStorage<_, PublicKey, _, _> =
        FileStorage::new(&profile.paths().keys_dir().join(KEY_FILE), pwhash);
    let keypair = file_storage.get_key()?;

    Ok(ZeroizingSecretKey::new(keypair.secret_key))
}

pub fn read_env_passphrase() -> Result<SecUtf8, anyhow::Error> {
    let env_var = std::env::var(RAD_PASSPHRASE)?;
    let input: Zeroizing<String> = Zeroizing::new(env_var);

    Ok(SecUtf8::from(input.trim_end()))
}

#[cfg(not(debug_assertions))]
pub fn pwhash(secret: SecUtf8) -> crypto::Pwhash<SecUtf8> {
    crypto::Pwhash::new(secret, crypto::KdfParams::recommended())
}

#[cfg(debug_assertions)]
pub fn pwhash(secret: SecUtf8) -> crypto::Pwhash<SecUtf8> {
    crypto::Pwhash::new(secret, *crypto::KDF_PARAMS_TEST)
}

pub mod radicle_keystore {

    use core::marker::PhantomData;
    use std::convert::TryFrom;
    use std::convert::TryInto;
    use std::ops::DerefMut;
    use std::path::{Path, PathBuf};

    use byteorder::BigEndian;
    use byteorder::ByteOrder;
    use byteorder::WriteBytesExt;

    use secstr::SecStr;
    use thiserror::Error;

    use radicle_ssh::encoding::Encoding;
    use radicle_ssh::encoding::{Buffer, Cursor};

    pub struct SigningKey(ed25519_zebra::SigningKey);

    impl From<ed25519_zebra::SigningKey> for SigningKey {
        fn from(key: ed25519_zebra::SigningKey) -> Self {
            Self(key)
        }
    }

    /// Ed25519 public key, encoded as per [RFC 8032]
    ///
    /// [RFC 8032]: https://tools.ietf.org/html/rfc8032
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub struct PublicKey(pub [u8; 32]);

    impl AsRef<[u8]> for PublicKey {
        fn as_ref(&self) -> &[u8] {
            &self.0
        }
    }

    impl From<&radicle::crypto::PublicKey> for PublicKey {
        fn from(key: &radicle::crypto::PublicKey) -> Self {
            Self(*key.0)
        }
    }

    #[derive(Debug, Error)]
    pub enum PublicKeyError {
        #[error("the public key parsed was not 32 bits in length")]
        Invalid,
        #[error(transparent)]
        Encoding(#[from] radicle_ssh::encoding::Error),
    }

    impl radicle_ssh::key::Public for PublicKey {
        type Error = PublicKeyError;

        fn read(r: &mut Cursor) -> Result<Option<Self>, Self::Error> {
            let t = r.read_string()?;
            match t {
                b"ssh-ed25519" => {
                    let p = r
                        .read_string()?
                        .try_into()
                        .map_err(|_| PublicKeyError::Invalid)?;
                    Ok(Some(Self(p)))
                }
                _ => Ok(None),
            }
        }

        fn write(&self, buf: &mut Buffer) -> usize {
            let mut str_w: Vec<u8> = Vec::<u8>::new();
            str_w.extend_ssh_string(b"ssh-ed25519");
            str_w.extend_ssh_string(&self.0[..]);
            buf.extend_ssh_string(&str_w)
        }
    }

    #[derive(Debug, Error)]
    pub enum SigningKeyError {
        #[error(transparent)]
        Encoding(#[from] radicle_ssh::encoding::Error),
        #[error(transparent)]
        Ed25519(#[from] ed25519_zebra::Error),
        #[error(transparent)]
        Io(#[from] io::Error),
    }

    impl radicle_ssh::key::Private for SigningKey {
        type Error = SigningKeyError;

        fn read(r: &mut Cursor) -> Result<Option<Self>, Self::Error> {
            let t = r.read_string()?;
            match t {
                b"ssh-ed25519" => {
                    let public_ = r.read_string()?;
                    let concat = r.read_string()?;
                    let _comment = r.read_string()?;
                    if &concat[32..64] != public_ {
                        return Ok(None);
                    }
                    let seed = &concat[0..32];
                    let key = SigningKey(ed25519_zebra::SigningKey::try_from(seed)?);
                    Ok(Some(key))
                }
                _ => Ok(None),
            }
        }

        fn write(&self, buf: &mut Buffer) -> Result<(), Self::Error> {
            let pk = ed25519_zebra::VerificationKey::from(&self.0);
            let seed = self.0.as_ref();
            let mut pair = [0u8; 64];
            pair[..32].copy_from_slice(seed);
            pair[32..].copy_from_slice(pk.as_ref());
            buf.extend_ssh_string(b"ssh-ed25519");
            buf.extend_ssh_string(pk.as_ref());
            buf.deref_mut().write_u32::<BigEndian>(64)?;
            buf.extend(&pair);
            // The GnuPG SSH agent fails to add keys with empty comments.
            // See: https://dev.gnupg.org/T5794
            buf.extend_ssh_string(b"radicle ed25519-zebra");
            Ok(())
        }

        fn write_signature<Bytes: AsRef<[u8]>>(
            &self,
            to_sign: Bytes,
            buf: &mut Buffer,
        ) -> Result<(), Self::Error> {
            let name = "ssh-ed25519";
            let signature: [u8; 64] = self.0.sign(to_sign.as_ref()).into();

            buf.deref_mut()
                .write_u32::<BigEndian>((name.len() + signature.len() + 8) as u32)?;
            buf.extend_ssh_string(name.as_bytes());
            buf.extend_ssh_string(&signature);
            Ok(())
        }
    }

    /// A [`ed25519::Signer`] backed by an `ssh-agent`.
    ///
    /// A connection to the agent needs to be established via [`SshAgent::connect`].
    /// Due to implementation limitations, the only way to connect is currently via
    /// the unix domain socket whose path is read from the `SSH_AUTH_SOCK`
    /// environment variable.
    pub struct SshAgent {
        key: PublicKey, //ed25519::PublicKey,
        path: Option<PathBuf>,
    }

    /// Ed25519 signature, encoded as per [RFC 8032]
    ///
    /// [RFC 8032]: https://tools.ietf.org/html/rfc8032
    #[derive(Clone, Copy)]
    pub struct Signature(pub [u8; 64]);

    pub trait SignerTrait {
        type Error: std::error::Error + Send + Sync + 'static;

        /// Obtain the [`PublicKey`] used for signing
        fn public_key(&self) -> PublicKey;

        /// Sign the supplied data with the secret key corresponding to
        /// [`Signer::public_key`]
        fn sign(&self, data: &[u8]) -> Result<Signature, Self::Error>;
    }

    use radicle_ssh::agent::client;
    pub mod error {
        use super::*;
        use thiserror::Error;

        #[derive(Debug, Error)]
        #[non_exhaustive]
        pub enum Connect {
            #[error(transparent)]
            Client(#[from] client::Error),
        }

        #[derive(Debug, Error)]
        #[non_exhaustive]
        pub enum AddKey {
            #[error(transparent)]
            Client(#[from] client::Error),
        }

        #[derive(Debug, Error)]
        #[non_exhaustive]
        pub enum RemoveKey {
            #[error(transparent)]
            Client(#[from] client::Error),
        }

        #[derive(Debug, Error)]
        #[non_exhaustive]
        pub enum ListKeys {
            #[error(transparent)]
            Client(#[from] client::Error),
        }

        #[derive(Debug, Error)]
        #[non_exhaustive]
        pub enum Sign {
            #[error(transparent)]
            Client(#[from] client::Error),
        }
    }

    use radicle_ssh::agent::client::AgentClient;
    use radicle_ssh::agent::client::ClientStream;
    use std::sync::Mutex;

    impl SshAgent {
        pub fn new(key: PublicKey) -> Self {
            Self { key, path: None }
        }

        pub fn with_path(self, path: PathBuf) -> Self {
            Self {
                path: Some(path),
                ..self
            }
        }

        /// Connects to the agent via a unix domain socket and provides a
        /// [`ed25519::Signer`] for signing a payload.
        ///
        /// If the path was set using [`SshAgent::with_path`], then that is used for
        /// the domain socket. Otherwise, the value of `SSH_AUTH_SOCKET` is used.
        ///
        /// # Note
        ///
        /// The stream parameter `S` needs to be chosen when calling this function.
        /// This is to leave the async runtime agnostic. The different
        /// implementations for streams can be found at [`ClientStream`]'s
        /// documentation.
        pub fn connect<S>(&self) -> Result<impl SignerTrait<Error = error::Sign>, error::Connect>
        where
            S: ClientStream + Unpin,
        {
            let client = self.client::<S>().map(|client| Mutex::new(Some(client)))?;

            Ok(Signer {
                rfc: self.key,
                client,
            })
        }

        fn client<S>(&self) -> Result<AgentClient<S>, client::Error>
        where
            S: ClientStream + Unpin,
        {
            match &self.path {
                None => Ok(S::connect_env()?),
                Some(path) => Ok(S::connect_socket(path)?),
            }
        }
    }

    // `AgentClient::sign_request_signature` returns `Result<(Self, Signature),
    // Error>` instead of `(Self, Result<Signature, Error>)`, which is probably a
    // bug. Because of this (and the move semantics, which are a bit weird anyways),
    // we need to slap our own mutex, and reconnect if we get an error.
    type Client<S> = Mutex<Option<AgentClient<S>>>;

    struct Signer<S> {
        rfc: PublicKey,
        client: Client<S>,
    }

    use zeroize::Zeroizing;

    impl<S> SignerTrait for Signer<S>
    where
        S: ClientStream + Unpin,
    {
        type Error = error::Sign;

        fn public_key(&self) -> PublicKey {
            self.rfc
        }

        fn sign(&self, data: &[u8]) -> Result<Signature, Self::Error> {
            let mut guard = self
                .client
                .lock()
                .map_err(|_| client::Error::AgentFailure)?;
            let mut client = match guard.take() {
                None => ClientStream::connect_env()?,
                Some(client) => client,
            };

            let sig = client.sign_request(&self.rfc, Zeroizing::new(data.to_vec()));
            *guard = Some(client);
            Ok(Signature(sig?))
        }
    }

    /// Class of types which can seal (encrypt) a secret, and unseal (decrypt) it
    /// from it's sealed form.
    ///
    /// It is up to the user to perform conversion from and to domain types.
    pub trait Crypto: Sized {
        type SecretBox;
        type Error;

        fn seal<K: AsRef<[u8]>>(&self, secret: K) -> Result<Self::SecretBox, Self::Error>;
        fn unseal(&self, secret_box: Self::SecretBox) -> Result<SecStr, Self::Error>;
    }

    /// [`Keystore`] implementation which stores the encrypted key in a file on the
    /// local filesystem.
    #[derive(Clone)]
    pub struct FileStorage<C, PK, SK, M> {
        key_file_path: PathBuf,
        crypto: C,

        _marker: PhantomData<(PK, SK, M)>,
    }

    impl<C, PK, SK, M> FileStorage<C, PK, SK, M> {
        /// Construct a new [`FileStorage`] with the given [`Crypto`]
        /// implementation.
        ///
        /// The [`Path`] given by `key_file_path` must be an actual file path, not a
        /// directory.
        pub fn new(key_file_path: &Path, crypto: C) -> Self {
            Self {
                key_file_path: key_file_path.to_path_buf(),
                crypto,

                _marker: PhantomData,
            }
        }

        /// [`Path`] to the file where the encrypted key is stored.
        pub fn key_file_path(&self) -> &Path {
            self.key_file_path.as_path()
        }
    }

    /// Named pair of public / secret key.
    pub struct Keypair<PK, SK> {
        pub public_key: PK,
        pub secret_key: SK,
    }

    pub trait SecretKeyExt: Sized {
        type Metadata;
        type Error;

        fn from_bytes_and_meta(
            bytes: SecStr,
            metadata: &Self::Metadata,
        ) -> Result<Self, Self::Error>;
        fn metadata(&self) -> Self::Metadata;
    }

    /// Abstraction over secure storage for private key material.
    pub trait Keystore {
        type PublicKey: From<Self::SecretKey>;
        type SecretKey: SecretKeyExt<Metadata = Self::Metadata>;

        type Metadata;

        type Error: std::error::Error;

        /// Securely store secret key `key` in the keystore.
        ///
        /// The key may carry [`Keystore::Metadata`], which is stored alongside the
        /// key material. The metadata, as well as the public portion of the
        /// key, may be stored in clear, so as to not require prompting the user
        /// when retrieving those values.
        ///
        /// Key rotation is not (yet) part of this API, thus `put_key` MUST return
        /// an error if an equivalent key is already present in the storage
        /// backend.
        fn put_key(&mut self, key: Self::SecretKey) -> Result<(), Self::Error>;

        /// Retrieve both the secret and public parts of the stored key material.
        fn get_key(&self) -> Result<Keypair<Self::PublicKey, Self::SecretKey>, Self::Error>;

        /// Retrieve only the public part of the key material, along with any
        /// metadata.
        fn show_key(&self) -> Result<(Self::PublicKey, Self::Metadata), Self::Error>;
    }

    use serde::{de::DeserializeOwned, Deserialize, Serialize};
    use std::fmt::{self, Debug, Display};
    use std::io;

    #[derive(Debug)]
    pub enum Error<Crypto, Conversion> {
        KeyExists(PathBuf),
        NoSuchKey(PathBuf),
        Crypto(Crypto),
        Conversion(Conversion),
        Serde(serde_cbor::error::Error),
        Io(io::Error),
    }

    impl<Crypto, Conversion> std::error::Error for Error<Crypto, Conversion>
    where
        Crypto: Display + Debug,
        Conversion: Display + Debug,
    {
    }

    impl<Crypto, Conversion> Display for Error<Crypto, Conversion>
    where
        Crypto: Display + Debug,
        Conversion: Display + Debug,
    {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Self::KeyExists(path) => {
                    write!(
                        f,
                        "Key exists at file path {}, refusing to overwrite",
                        path.display()
                    )
                }
                Self::NoSuchKey(path) => write!(f, "No key found at file path: {}", path.display()),
                Self::Conversion(e) => write!(f, "Error reconstructing sealed key: {}", e),
                Self::Crypto(e) => write!(f, "Error unsealing key: {}", e),
                Self::Serde(e) => write!(f, "{}", e),
                Self::Io(e) => write!(f, "{}", e),
            }
        }
    }

    impl<Crypto, Conversion> From<io::Error> for Error<Crypto, Conversion> {
        fn from(e: io::Error) -> Self {
            Self::Io(e)
        }
    }

    impl<Crypto, Conversion> From<serde_cbor::error::Error> for Error<Crypto, Conversion> {
        fn from(e: serde_cbor::error::Error) -> Self {
            Self::Serde(e)
        }
    }

    use std::fs::File;

    #[derive(Serialize, Deserialize)]
    struct Stored<PK, S, M> {
        public_key: PK,
        secret_key: S,
        metadata: M,
    }

    impl<C, PK, SK, M> Keystore for FileStorage<C, PK, SK, M>
    where
        C: Crypto,
        C::Error: Display + Debug,
        C::SecretBox: Serialize + DeserializeOwned,

        SK: AsRef<[u8]> + SecretKeyExt<Metadata = M>,
        <SK as SecretKeyExt>::Error: Display + Debug,

        PK: Clone + From<SK> + Serialize + DeserializeOwned,
        M: Clone + Serialize + DeserializeOwned,
    {
        type PublicKey = PK;
        type SecretKey = SK;
        type Metadata = M;
        type Error = Error<C::Error, <SK as SecretKeyExt>::Error>;

        fn put_key(&mut self, key: Self::SecretKey) -> Result<(), Self::Error> {
            if self.key_file_path().exists() {
                return Err(Error::KeyExists(self.key_file_path.clone()));
            }

            let metadata = key.metadata();
            let sealed_key = self.crypto.seal(&key).map_err(Error::Crypto)?;

            let key_file = File::create(self.key_file_path())?;
            serde_cbor::to_writer(
                &key_file,
                &Stored {
                    public_key: Self::PublicKey::from(key),
                    secret_key: sealed_key,
                    metadata,
                },
            )?;
            key_file.sync_all()?;

            Ok(())
        }

        fn get_key(&self) -> Result<Keypair<Self::PublicKey, Self::SecretKey>, Self::Error> {
            if !self.key_file_path().exists() {
                return Err(Error::NoSuchKey(self.key_file_path.clone()));
            }

            let stored: Stored<Self::PublicKey, <C as Crypto>::SecretBox, Self::Metadata> =
                serde_cbor::from_reader(File::open(self.key_file_path())?)?;

            let secret_key = {
                let sbox = stored.secret_key;
                let meta = stored.metadata;

                self.crypto
                    .unseal(sbox)
                    .map_err(Error::Crypto)
                    .and_then(|sec| {
                        Self::SecretKey::from_bytes_and_meta(sec, &meta).map_err(Error::Conversion)
                    })
            }?;

            Ok(Keypair {
                public_key: stored.public_key,
                secret_key,
            })
        }

        fn show_key(&self) -> Result<(Self::PublicKey, Self::Metadata), Self::Error> {
            if !self.key_file_path().exists() {
                return Err(Error::NoSuchKey(self.key_file_path.clone()));
            }

            let stored: Stored<Self::PublicKey, <C as Crypto>::SecretBox, Self::Metadata> =
                serde_cbor::from_reader(File::open(self.key_file_path())?)?;

            Ok((stored.public_key, stored.metadata))
        }
    }

    use radicle_ssh::agent::Constraint;

    /// Add a secret key to a running ssh-agent.
    ///
    /// Connects to the agent via the `SSH_AUTH_SOCK` unix domain socket.
    ///
    /// # Note
    ///
    /// The stream parameter `S` needs to be chosen when calling this function. This
    /// is to leave the async runtime agnostic. The different implementations for
    /// streams can be found at [`ClientStream`]'s documentation.
    pub fn add_key<S>(
        agent: &SshAgent,
        secret: ed25519_zebra::SigningKey,
        constraints: &[Constraint],
    ) -> Result<(), error::AddKey>
    where
        S: ClientStream + Unpin,
    {
        let mut client = agent.client::<S>()?;
        let secret = SigningKey::from(secret);
        client.add_identity(&secret, constraints)?;

        Ok(())
    }

    pub fn remove_key<S>(agent: &SshAgent, key: &PublicKey) -> Result<(), error::RemoveKey>
    where
        S: ClientStream + Unpin,
    {
        let mut client = agent.client::<S>()?;
        client.remove_identity(key)?;
        Ok(())
    }

    pub fn list_keys<S>(agent: &SshAgent) -> Result<Vec<PublicKey>, error::ListKeys>
    where
        S: ClientStream + Unpin,
    {
        let mut client = agent.client::<S>()?;
        let keys = client.request_identities()?;
        Ok(keys)
    }

    //
    //
    //
    use std::convert::Infallible;

    use rpassword::read_password_from_tty;
    pub use secstr::SecUtf8;

    /// A method to obtain a passphrase from which an encryption key can be derived.
    ///
    /// Similar in spirit to GPG's `pinentry` program, but no implementation of the
    /// Assuan protocol is provided as of yet.
    pub trait Pinentry {
        type Error;

        fn get_passphrase(&self) -> Result<SecUtf8, Self::Error>;
    }

    /// Identity [`Pinentry`]
    impl Pinentry for SecUtf8 {
        type Error = Infallible;

        fn get_passphrase(&self) -> Result<SecUtf8, Infallible> {
            Ok(self.clone())
        }
    }

    /// [`Pinentry`] which prompts the user on the TTY
    pub struct Prompt<'a>(&'a str);

    impl<'a> Prompt<'a> {
        pub fn new(prompt: &'a str) -> Self {
            Self(prompt)
        }
    }

    impl<'a> Pinentry for Prompt<'a> {
        type Error = io::Error;

        fn get_passphrase(&self) -> Result<SecUtf8, Self::Error> {
            read_password_from_tty(Some(self.0)).map(SecUtf8::from)
        }
    }

    //
    //
    //

    use chacha20poly1305::{aead, aead::Aead, KeyInit};
    use generic_array::GenericArray;
    use lazy_static::lazy_static;

    lazy_static! {
        /// [`KdfParams`] suitable for production use.
        pub static ref KDF_PARAMS_PROD: KdfParams = scrypt::Params::new(15, 8, 1).unwrap();

        /// [`KdfParams`] suitable for use in tests.
        ///
        /// # Warning
        ///
        /// These parameters allows a brute-force attack against an encrypted
        /// [`SecretBox`] to be carried out at significantly lower cost. Care must
        /// be taken by users of this library to prevent accidental use of test
        /// parameters in a production setting.
        pub static ref KDF_PARAMS_TEST: KdfParams = scrypt::Params::new(4, 8, 1).unwrap();
    }

    /// Parameters for the key derivation function.
    pub type KdfParams = scrypt::Params;

    /// Nonce used for secret box.
    type Nonce =
        GenericArray<u8, <chacha20poly1305::ChaCha20Poly1305 as aead::AeadCore>::NonceSize>;

    /// Size of the salt, in bytes.
    const SALT_SIZE: usize = 24;

    /// 192-bit salt.
    type Salt = [u8; SALT_SIZE];

    #[derive(Clone, Serialize, Deserialize)]
    pub struct SecretBox {
        nonce: Nonce,
        salt: Salt,
        sealed: Vec<u8>,
    }

    #[derive(Debug, Error)]
    pub enum SecretBoxError<PinentryError: std::error::Error + 'static> {
        #[error("Unable to decrypt secret box using the derived key")]
        InvalidKey,

        #[error("Error returned from underlying crypto")]
        CryptoError,

        #[error("Error getting passphrase")]
        Pinentry(#[from] PinentryError),
    }

    /// A [`Crypto`] implementation based on `libsodium`'s "secretbox".
    ///
    /// While historically based on `libsodium`, the underlying implementation is
    /// now based on the [`chacha20poly1305`] crate. The encryption key is derived
    /// from a passphrase using [`scrypt`].
    ///
    /// The resulting [`SecretBox`] stores the ciphertext alongside cleartext salt
    /// and nonce values.
    #[derive(Clone)]
    pub struct Pwhash<P> {
        pinentry: P,
        params: KdfParams,
    }

    impl<P> Pwhash<P> {
        /// Create a new [`Pwhash`] value
        pub fn new(pinentry: P, params: KdfParams) -> Self {
            Self { pinentry, params }
        }
    }

    impl<P> Crypto for Pwhash<P>
    where
        P: Pinentry,
        P::Error: std::error::Error + 'static,
    {
        type SecretBox = SecretBox;
        type Error = SecretBoxError<P::Error>;

        fn seal<K: AsRef<[u8]>>(&self, secret: K) -> Result<Self::SecretBox, Self::Error> {
            use rand::RngCore;

            let passphrase = self
                .pinentry
                .get_passphrase()
                .map_err(SecretBoxError::Pinentry)?;

            let mut rng = rand::thread_rng();

            // Generate nonce.
            let mut nonce = [0; 12];
            rng.fill_bytes(&mut nonce);

            // Generate salt.
            let mut salt: Salt = [0; SALT_SIZE];
            rng.fill_bytes(&mut salt);

            // Derive key from passphrase.
            let nonce = *Nonce::from_slice(&nonce[..]);
            let derived = derive_key(&salt, &passphrase, &self.params);
            let key = chacha20poly1305::Key::from_slice(&derived[..]);
            let cipher = chacha20poly1305::ChaCha20Poly1305::new(key);

            let sealed = cipher
                .encrypt(&nonce, secret.as_ref())
                .map_err(|_| Self::Error::CryptoError)?;

            Ok(SecretBox {
                nonce,
                salt,
                sealed,
            })
        }

        fn unseal(&self, secret_box: Self::SecretBox) -> Result<SecStr, Self::Error> {
            let passphrase = self
                .pinentry
                .get_passphrase()
                .map_err(SecretBoxError::Pinentry)?;

            let derived = derive_key(&secret_box.salt, &passphrase, &self.params);
            let key = chacha20poly1305::Key::from_slice(&derived[..]);
            let cipher = chacha20poly1305::ChaCha20Poly1305::new(key);

            cipher
                .decrypt(&secret_box.nonce, secret_box.sealed.as_slice())
                .map_err(|_| SecretBoxError::InvalidKey)
                .map(SecStr::new)
        }
    }

    fn derive_key(salt: &Salt, passphrase: &SecUtf8, params: &KdfParams) -> [u8; 32] {
        let mut key = [0u8; 32];
        scrypt::scrypt(passphrase.unsecure().as_bytes(), salt, params, &mut key)
            .expect("Output length must not be zero");

        key
    }
}

pub mod ssh_super {

    use core::convert::Infallible;
    use core::str::FromStr;
    use std::fmt;
    use std::path::{Path, PathBuf};

    use super::radicle_keystore::SshAgent;

    /// Which unix domain socket the `ssh-agent` should connect to.
    ///
    /// When this value is `Env` it will use the `SSH_AUTH_SOCK` environment
    /// variable. When this value is `Uds` it will use the path provided.
    ///
    /// # Default
    ///
    /// The default value for this `Env`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum SshAuthSock {
        Env,
        Uds(PathBuf),
    }

    impl fmt::Display for SshAuthSock {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Env => write!(f, "env"),
                Self::Uds(path) => write!(f, "{}", path.display()),
            }
        }
    }

    impl FromStr for SshAuthSock {
        type Err = Infallible;

        fn from_str(val: &str) -> Result<Self, Self::Err> {
            match val {
                "env" => Ok(Self::Env),
                s => Ok(Self::Uds(Path::new(s).to_path_buf())),
            }
        }
    }

    impl Default for SshAuthSock {
        fn default() -> Self {
            Self::Env
        }
    }

    pub fn with_socket(agent: SshAgent, sock: SshAuthSock) -> SshAgent {
        match sock {
            SshAuthSock::Env => agent,
            SshAuthSock::Uds(path) => agent.with_path(path),
        }
    }
}

pub mod link_crypto {
    use std::convert::TryFrom;
    use std::ops::Deref;

    use ed25519_zebra as ed25519;
    use secstr::SecStr;
    use thiserror::Error;
    use zeroize::Zeroize;

    /// A device-specific signing key
    #[derive(Clone, Zeroize)]
    #[cfg_attr(test, derive(Debug))]
    #[zeroize(drop)]
    pub struct SecretKey(ed25519::SigningKey);

    /// A signature produced by `Key::sign`
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct Signature(ed25519::Signature);

    impl From<sign::Signature> for Signature {
        fn from(other: sign::Signature) -> Signature {
            Signature(ed25519::Signature::from(other.0))
        }
    }

    impl From<Signature> for [u8; 64] {
        fn from(sig: Signature) -> [u8; 64] {
            sig.0.into()
        }
    }

    impl Deref for Signature {
        type Target = ed25519::Signature;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    #[allow(clippy::new_without_default)]
    impl SecretKey {
        pub fn new() -> Self {
            let sk = ed25519::SigningKey::new(rand::thread_rng());
            Self(sk)
        }

        pub fn from_seed(seed: [u8; 32]) -> Self {
            Self(ed25519::SigningKey::from(seed))
        }

        pub(crate) fn from_secret(sk: ed25519::SigningKey) -> Self {
            Self(sk)
        }

        pub fn public(&self) -> PublicKey {
            PublicKey(ed25519::VerificationKeyBytes::from(
                ed25519::VerificationKey::from(&self.0),
            ))
        }

        pub fn sign(&self, data: &[u8]) -> Signature {
            Signature(self.0.sign(data))
        }
    }

    impl AsRef<[u8]> for SecretKey {
        fn as_ref(&self) -> &[u8] {
            self.0.as_ref()
        }
    }

    impl Signer for SecretKey {
        fn sign_blocking(
            &self,
            data: &[u8],
        ) -> Result<sign::Signature, <Self as sign::SignerTrait>::Error> {
            let sig = SecretKey::sign(self, data);
            Ok(sign::Signature(sig.into()))
        }
    }

    use std::convert::Infallible;

    impl sign::SignerTrait for SecretKey {
        type Error = Infallible;

        fn public_key(&self) -> sign::PublicKey {
            sign::SignerTrait::public_key(&self)
        }

        fn sign(&self, data: &[u8]) -> Result<sign::Signature, Self::Error> {
            sign::SignerTrait::sign(&self, data)
        }
    }

    impl<'a> sign::SignerTrait for &'a SecretKey {
        type Error = Infallible;

        fn public_key(&self) -> sign::PublicKey {
            sign::PublicKey(ed25519::VerificationKey::from(&self.0).into())
        }

        fn sign(&self, data: &[u8]) -> Result<sign::Signature, Self::Error> {
            let signature = (*self).sign(data).0;
            Ok(sign::Signature(signature.into()))
        }
    }

    impl From<SecretKey> for ed25519::SigningKey {
        fn from(key: SecretKey) -> Self {
            key.0
        }
    }

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum IntoSecretKeyError {
        #[error("invalid length")]
        InvalidSliceLength,
    }

    impl super::radicle_keystore::SecretKeyExt for SecretKey {
        type Metadata = ();
        type Error = IntoSecretKeyError;

        fn from_bytes_and_meta(
            bytes: SecStr,
            _metadata: &Self::Metadata,
        ) -> Result<Self, Self::Error> {
            let sk = ed25519::SigningKey::try_from(bytes.unsecure())
                .map_err(|_| IntoSecretKeyError::InvalidSliceLength)?;
            Ok(Self::from_secret(sk))
        }

        fn metadata(&self) -> Self::Metadata {}
    }

    /// The public part of a `Key``
    #[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
    pub struct PublicKey(ed25519::VerificationKeyBytes);

    impl From<super::radicle_keystore::PublicKey> for PublicKey {
        fn from(other: super::radicle_keystore::PublicKey) -> PublicKey {
            PublicKey(ed25519::VerificationKeyBytes::from(other.0))
        }
    }

    impl From<PublicKey> for super::radicle_keystore::PublicKey {
        fn from(other: PublicKey) -> Self {
            Self(other.0.into())
        }
    }

    impl From<SecretKey> for PublicKey {
        fn from(k: SecretKey) -> Self {
            k.public()
        }
    }

    impl AsRef<[u8]> for PublicKey {
        fn as_ref(&self) -> &[u8] {
            self.0.as_ref()
        }
    }

    use multibase::Base;
    use serde::{Serialize, Serializer};
    use std::iter;

    /// Version of the signature scheme in use
    ///
    /// This is used for future-proofing serialisation. For ergonomics reasons, we
    /// avoid introducing single-variant enums just now, and just serialize a
    /// version tag alongside the data.
    const VERSION: u8 = 0;

    impl Serialize for PublicKey {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            multibase::encode(
                Base::Base32Z,
                iter::once(&VERSION)
                    .chain(self.as_ref())
                    .cloned()
                    .collect::<Vec<u8>>(),
            )
            .serialize(serializer)
        }
    }

    use serde::de::Visitor;
    use serde::{Deserialize, Deserializer};
    use std::fmt;

    impl<'de> Deserialize<'de> for PublicKey {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct PublicKeyVisitor;

            impl<'de> Visitor<'de> for PublicKeyVisitor {
                type Value = PublicKey;

                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    write!(f, "a PublicKey, version {}", VERSION)
                }

                fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    let (_, bytes) = multibase::decode(s).map_err(serde::de::Error::custom)?;
                    match bytes.split_first() {
                        // impossible, actually
                        None => Err(serde::de::Error::custom("Empty input")),
                        Some((version, data)) => {
                            if version != &VERSION {
                                return Err(serde::de::Error::custom(format!(
                                    "Unknown PublicKey version {}",
                                    version
                                )));
                            }

                            ed25519::VerificationKeyBytes::try_from(data)
                                .map(PublicKey)
                                .map_err(serde::de::Error::custom)
                        }
                    }
                }
            }

            deserializer.deserialize_str(PublicKeyVisitor)
        }
    }

    /// A blanket trait over [`sign::Signer`] that can be shared safely among
    /// threads.
    /// NOTE: might be redundant since `.sign` became sync
    pub trait Signer:
        super::radicle_keystore::SignerTrait + Send + Sync + dyn_clone::DynClone + 'static
    {
        fn sign_blocking(
            &self,
            data: &[u8],
        ) -> Result<
            super::radicle_keystore::Signature,
            <Self as super::radicle_keystore::SignerTrait>::Error,
        > {
            self.sign(data)
        }
    }

    /// A dynamic [`Signer`] where the associated error is a [`BoxedSignError`].
    /// This allows us to dynamically send around something that is a `Signer`. This
    /// is important for `librad`'s `git::local::transport`.
    pub struct BoxedSigner {
        signer: Box<dyn Signer<Error = BoxedSignError>>,
    }

    impl BoxedSigner {
        /// Create a new `BoxedSigner` from any [`Signer`].
        pub fn new<S>(signer: S) -> Self
        where
            S: Signer<Error = BoxedSignError>,
        {
            BoxedSigner {
                signer: Box::new(signer),
            }
        }
    }

    impl From<SecretKey> for BoxedSigner {
        fn from(key: SecretKey) -> Self {
            Self::from(SomeSigner { signer: key })
        }
    }

    impl Clone for BoxedSigner {
        fn clone(&self) -> Self {
            BoxedSigner {
                signer: dyn_clone::clone_box(&*self.signer),
            }
        }
    }

    impl Signer for BoxedSigner {
        fn sign_blocking(
            &self,
            data: &[u8],
        ) -> Result<sign::Signature, <Self as sign::SignerTrait>::Error> {
            self.signer.sign_blocking(data)
        }
    }

    impl sign::SignerTrait for BoxedSigner {
        type Error = BoxedSignError;

        fn public_key(&self) -> sign::PublicKey {
            self.signer.public_key()
        }

        fn sign(&self, data: &[u8]) -> Result<sign::Signature, Self::Error> {
            self.signer.sign(data)
        }
    }

    /// An implementation of `sign::Signer` will have a concrete associated `Error`.
    /// If we would like to use it as a `BoxedSigner` then we need to create an
    /// implementation of `sign::Signer` which uses `BoxedSignError`.
    ///
    /// We can do this generically over any `S` that implements `sign::Signer` with
    /// and associated `Error` that implementat `std::error::Error`.
    #[derive(Clone)]
    pub struct SomeSigner<S> {
        pub signer: S,
    }

    use super::radicle_keystore as sign;

    impl<S: Signer + Clone> Signer for SomeSigner<S> {
        fn sign_blocking(
            &self,
            data: &[u8],
        ) -> Result<super::radicle_keystore::Signature, <Self as sign::SignerTrait>::Error>
        {
            self.signer
                .sign_blocking(data)
                .map_err(BoxedSignError::from_std_error)
        }
    }

    impl<S> From<SomeSigner<S>> for BoxedSigner
    where
        S: Signer + Clone + Send + Sync + 'static,
        S::Error: std::error::Error + Send + Sync + 'static,
    {
        fn from(other: SomeSigner<S>) -> Self {
            BoxedSigner::new(other)
        }
    }

    impl<S> sign::SignerTrait for SomeSigner<S>
    where
        S: sign::SignerTrait + Clone + Send + Sync + 'static,
        S::Error: std::error::Error + Send + Sync + 'static,
    {
        type Error = BoxedSignError;

        fn public_key(&self) -> sign::PublicKey {
            self.signer.public_key()
        }

        fn sign(&self, data: &[u8]) -> Result<sign::Signature, Self::Error> {
            sign::SignerTrait::sign(&self.signer, data).map_err(BoxedSignError::from_std_error)
        }
    }

    /// A boxed [`Error`] that is used as the associated `Error` type for
    /// [`BoxedSigner`].
    pub struct BoxedSignError {
        error: Box<dyn std::error::Error + Send + Sync + 'static>,
    }

    impl BoxedSignError {
        /// Turn any [`Error`] into a `BoxedSignError`.
        pub fn from_std_error<T>(other: T) -> Self
        where
            T: std::error::Error + Send + Sync + 'static,
        {
            BoxedSignError {
                error: Box::new(other),
            }
        }
    }

    impl std::fmt::Debug for BoxedSignError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.error)
        }
    }

    impl std::fmt::Display for BoxedSignError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.error)
        }
    }

    impl std::error::Error for BoxedSignError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(self)
        }

        fn cause(&self) -> Option<&dyn std::error::Error> {
            Some(self)
        }
    }
}

/// TODO(dave): move this mod to heartwood
pub mod ssh {
    use std::fmt;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;

    use radicle::profile::Profile;
    use radicle::Storage;
    use radicle_ssh::agent::Constraint;
    use serde::{de::DeserializeOwned, Serialize};
    use thiserror::Error;

    use super::link_crypto::{BoxedSignError, BoxedSigner, PublicKey, SecretKey};
    use super::radicle_keystore as ssh;
    use super::radicle_keystore::SignerTrait;
    use super::radicle_keystore::{add_key, list_keys, remove_key};
    use super::radicle_keystore::{Crypto, FileStorage, Keystore, SshAgent};
    use super::ssh_super::{with_socket, SshAuthSock};

    #[derive(Clone)]
    pub struct SshSigner {
        signer:
            Arc<dyn super::radicle_keystore::SignerTrait<Error = ssh::error::Sign> + Send + Sync>,
    }

    impl super::radicle_keystore::SignerTrait for SshSigner {
        type Error = BoxedSignError;

        fn public_key(&self) -> super::radicle_keystore::PublicKey {
            self.signer.public_key()
        }

        fn sign(&self, data: &[u8]) -> Result<super::radicle_keystore::Signature, BoxedSignError> {
            self.signer
                .sign(data)
                .map_err(BoxedSignError::from_std_error)
        }
    }

    impl super::link_crypto::Signer for SshSigner {
        fn sign_blocking(
            &self,
            data: &[u8],
        ) -> Result<
            super::radicle_keystore::Signature,
            <Self as super::radicle_keystore::SignerTrait>::Error,
        > {
            let data = data.to_vec();
            self.sign(&data)
        }
    }

    #[derive(Debug, Error)]
    pub enum Error {
        #[error(transparent)]
        AddKey(#[from] ssh::error::AddKey),
        #[error("failed to get the key material from your file storage")]
        GetKey(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
        #[error(transparent)]
        ListKeys(#[from] ssh::error::ListKeys),
        #[error("the key is not in the ssh-agent")]
        NoSuchKey,
        #[error(transparent)]
        RemoveKey(#[from] ssh::error::RemoveKey),
        #[error(transparent)]
        SignError(#[from] BoxedSignError),
        #[error(transparent)]
        SshConnect(#[from] ssh::error::Connect),
        #[error(transparent)]
        IoError(#[from] std::io::Error),
        // TODO(dave): find `radicle` equivalent
        //#[error(transparent)]
        //StorageInit(#[from] read::error::Init),
    }

    /// Get the signing key associated with this `profile`.
    /// See [`SshAuthSock`] for how the `ssh-agent` will be connected to. Use
    /// `SshAuthSock::default` to connect via `SSH_AUTH_SOCK`.
    pub fn signer(profile: &Profile, sock: SshAuthSock) -> Result<BoxedSigner, Error> {
        // TODO(dave) substitute for ReadOnly
        //let storage = ReadOnly::open(profile.paths())?;
        let storage = Storage::open(profile.paths().storage())?;
        let pk = profile.id().into();
        let agent = with_socket(SshAgent::new(pk), sock);
        let keys = list_keys::<UnixStream>(&agent)?;
        if keys.contains(&pk) {
            let signer = agent.connect::<UnixStream>()?;
            let signer = SshSigner {
                signer: Arc::new(signer),
            };
            Ok(BoxedSigner::new(signer))
        } else {
            Err(Error::NoSuchKey)
        }
    }

    /// Add the signing key associated with this `profile` to the `ssh-agent`.
    ///
    /// See [`SshAuthSock`] for how the agent will be connected to. Use
    /// `SshAuthSock::default` to connect via `SSH_AUTH_SOCK`.
    ///
    /// The `crypto` passed will decide how the key storage is unlocked.
    pub fn add_signer<C>(
        profile: &Profile,
        sock: SshAuthSock,
        crypto: C,
        constraints: Vec<Constraint>,
    ) -> Result<(), super::Error>
    where
        C: Crypto,
        C::Error: fmt::Debug + fmt::Display + Send + Sync + 'static,
        C::SecretBox: Serialize + DeserializeOwned,
    {
        let store = file_storage(profile, crypto);
        let key = store.get_key().map_err(|err| Error::GetKey(err.into()))?;
        let agent = with_socket(SshAgent::new(key.public_key.into()), sock);
        add_key::<UnixStream>(&agent, key.secret_key.into(), &constraints)?;
        Ok(())
    }

    /// Remove the signing key associated with this `profile` from the `ssh-agent`.
    ///
    /// See [`SshAuthSock`] for how the agent will be connected to. Use
    /// `SshAuthSock::default` to connect via `SSH_AUTH_SOCK`.
    ///
    /// The `crypto` passed will decide how the key storage is unlocked.
    pub fn remove_signer<C>(
        profile: &Profile,
        sock: SshAuthSock,
        crypto: C,
    ) -> Result<(), super::Error>
    where
        C: Crypto,
        C::Error: fmt::Debug + fmt::Display + Send + Sync + 'static,
        C::SecretBox: Serialize + DeserializeOwned,
    {
        let store = file_storage(profile, crypto);
        let key = store.get_key().map_err(|err| Error::GetKey(err.into()))?;
        let agent = with_socket(SshAgent::new(key.public_key.into()), sock);
        Ok(ssh::remove_key::<UnixStream>(
            &agent,
            &key.public_key.into(),
        )?)
    }

    /// Test whether the signing key associated with this `profile` is present on
    /// the `ssh-agent`.
    ///
    /// See [`SshAuthSock`] for how the agent will be connected to. Use
    /// `SshAuthSock::default` to connect via `SSH_AUTH_SOCK`.
    pub fn is_signer_present(profile: &Profile, sock: SshAuthSock) -> Result<bool, super::Error> {
        let storage = Storage::open(profile.paths().storage())?;
        let pk = profile.id().into();
        let agent = with_socket(SshAgent::new(pk), sock);
        let keys = ssh::list_keys::<UnixStream>(&agent)?;
        Ok(keys.contains(&pk))
    }

    /// The filename for storing the secret key.
    pub const LIBRAD_KEY_FILE: &str = "librad.key";

    /// Create a [`FileStorage`] for [`SecretKey`]s.
    fn file_storage<C>(profile: &Profile, crypto: C) -> FileStorage<C, PublicKey, SecretKey, ()>
    where
        C: Crypto,
    {
        FileStorage::new(&profile.paths().keys_dir().join(LIBRAD_KEY_FILE), crypto)
    }
}
