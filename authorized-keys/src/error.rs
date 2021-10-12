#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),

    #[error(transparent)]
    Git2Error(#[from] git2::Error),

    #[error(transparent)]
    PgpError(#[from] pgp::errors::Error),

    #[error("key already exists in keyring")]
    KeyExists,

    #[error("key does not exist in keyring")]
    KeyDoesNotExist,

    #[error("authorized keys files not found. manually add the file or use the CLI `-i` flag to initialize the file")]
    AccessControlFileNotFound,

    #[error("keyring source is not supported")]
    UnsupportedKeyRingSource,

    #[error("key type is unsupported")]
    UnsupportedKey,

    #[error("missing key id")]
    MissingKeyId,

    #[error(transparent)]
    TimeoutError(#[from] std::sync::mpsc::RecvTimeoutError),

    #[error(transparent)]
    NoStdInFound(#[from] std::sync::mpsc::SendError<String>),

    #[error("keys directory not found")]
    MissingKeysDirectory,
}

pub const TIMEOUT_STDIN_WARNING: &str = r#" 
        Expected public key source from standard input.


        Try piping your public key, e.g.:

        `gpg --armor --export <your@email.address> | rad-auth-keys add`


        Alternatively,


        Use the `--path` flag to set the path to your "publickey.pub" file, e.g.:

        `rad-auth-keys add -p ./path/to/key.pub`

"#;
