use ethers::prelude::{maybe, PendingTransaction};
use ethers::providers::{FromErr, Middleware};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::transaction::eip712::Eip712;
use ethers::types::{Address, BlockId, Bytes, Signature, H256};

use async_trait::async_trait;
use thiserror::Error;

/// Trait for signing transactions and messages
///
/// Implement this trait to support different signing modes, e.g. Ledger, hosted etc.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait Signer: std::fmt::Debug + Send + Sync {
    type Error: std::error::Error + Send + Sync;
    /// Signs the hash of the provided message after prefixing it
    async fn sign_message<S: Send + Sync + AsRef<[u8]>>(
        &self,
        message: S,
    ) -> Result<Signature, Self::Error>;

    /// Signs the transaction
    async fn sign_transaction(&self, message: &TypedTransaction) -> Result<Signature, Self::Error>;

    /// Sends the transaction (only applicable for walletconnect)
    async fn send_transaction(&self, message: &TypedTransaction) -> Result<H256, Self::Error>;

    /// Encodes and signs the typed data according EIP-712.
    /// Payload must implement Eip712 trait.
    async fn sign_typed_data<T: Eip712 + Send + Sync>(
        &self,
        payload: &T,
    ) -> Result<Signature, Self::Error>;

    /// Returns the signer's Ethereum Address
    fn address(&self) -> Address;

    /// Returns the signer's chain id
    fn chain_id(&self) -> u64;

    /// Sets the signer's chain id
    fn with_chain_id<T: Into<u64>>(self, chain_id: T) -> Self;

    /// Check if signer is a WalletConnect signer
    fn is_walletconnect(&self) -> bool;
}

#[derive(Debug)]
pub struct SignerMiddleware<M, S> {
    pub inner: M,
    pub signer: S,
    pub address: Address,
    pub is_legacy: bool,
    pub is_walletconnect: bool,
}

impl<M: Middleware, S: Signer> FromErr<M::Error> for SignerMiddlewareError<M, S> {
    fn from(src: M::Error) -> SignerMiddlewareError<M, S> {
        SignerMiddlewareError::MiddlewareError(src)
    }
}

#[derive(Error, Debug)]
/// Error thrown when the client interacts with the blockchain
pub enum SignerMiddlewareError<M: Middleware, S: Signer> {
    #[error("{0}")]
    /// Thrown when the internal call to the signer fails
    SignerError(S::Error),

    #[error("{0}")]
    /// Thrown when an internal middleware errors
    MiddlewareError(M::Error),

    /// Thrown if the `nonce` field is missing
    #[error("no nonce was specified")]
    NonceMissing,
    /// Thrown if the `gas_price` field is missing
    #[error("no gas price was specified")]
    GasPriceMissing,
    /// Thrown if the `gas` field is missing
    #[error("no gas was specified")]
    GasMissing,
    /// Thrown if a signature is requested from a different address
    #[error("specified from address is not signer")]
    WrongSigner,
}

// Helper functions for locally signing transactions
impl<M, S> SignerMiddleware<M, S>
where
    M: Middleware,
    S: Signer,
{
    /// Creates a new client from the provider and signer.
    pub fn new(inner: M, signer: S) -> Self {
        let is_legacy = std::env::var_os("RAD_SIGNER_LEGACY").is_some();
        let is_walletconnect = signer.is_walletconnect();
        let address = signer.address();
        SignerMiddleware {
            inner,
            signer,
            address,
            is_legacy,
            is_walletconnect,
        }
    }

    /// Signs and returns the RLP encoding of the signed transaction
    async fn sign_transaction(
        &self,
        tx: TypedTransaction,
    ) -> Result<Bytes, SignerMiddlewareError<M, S>> {
        let signature = self
            .signer
            .sign_transaction(&tx)
            .await
            .map_err(SignerMiddlewareError::SignerError)?;

        // Return the raw rlp-encoded signed transaction
        Ok(tx.rlp_signed(self.signer.chain_id(), &signature))
    }

    /// Returns the client's address
    pub fn address(&self) -> Address {
        self.address
    }

    /// Returns a reference to the client's signer
    pub fn signer(&self) -> &S {
        &self.signer
    }

    /*
    pub fn with_signer(&self, signer: S) -> Self
    where
        S: Clone,
        M: Clone,
    {
        let mut this = self.clone();
        this.address = signer.address();
        this.signer = signer;
        this
    }
    */
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<M, S> Middleware for SignerMiddleware<M, S>
where
    M: Middleware,
    S: Signer,
{
    type Error = SignerMiddlewareError<M, S>;
    type Provider = M::Provider;
    type Inner = M;

    fn inner(&self) -> &M {
        &self.inner
    }

    /// Returns the client's address
    fn default_sender(&self) -> Option<Address> {
        Some(self.address)
    }

    /// `SignerMiddleware` is instantiated with a signer.
    async fn is_signer(&self) -> bool {
        true
    }

    async fn sign_transaction(
        &self,
        tx: &TypedTransaction,
        _: Address,
    ) -> Result<Signature, Self::Error> {
        Ok(self
            .signer
            .sign_transaction(tx)
            .await
            .map_err(SignerMiddlewareError::SignerError)?)
    }

    /// Helper for filling a transaction's nonce using the wallet
    async fn fill_transaction(
        &self,
        tx: &mut TypedTransaction,
        block: Option<BlockId>,
    ) -> Result<(), Self::Error> {
        // get the `from` field's nonce if it's set, else get the signer's nonce
        let from = if tx.from().is_some() && tx.from() != Some(&self.address()) {
            *tx.from().unwrap()
        } else {
            self.address
        };
        tx.set_from(from);

        let nonce = maybe(tx.nonce().cloned(), self.get_transaction_count(from, block)).await?;
        tx.set_nonce(nonce);
        self.inner()
            .fill_transaction(tx, block)
            .await
            .map_err(SignerMiddlewareError::MiddlewareError)?;
        Ok(())
    }

    /// Signs and broadcasts the transaction. The optional parameter `block` can be passed so that
    /// gas cost and nonce calculations take it into account. For simple transactions this can be
    /// left to `None`.
    async fn send_transaction<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        tx: T,
        block: Option<BlockId>,
    ) -> Result<PendingTransaction<'_, Self::Provider>, Self::Error> {
        let mut tx = tx.into();

        // fill any missing fields
        self.fill_transaction(&mut tx, block).await?;

        // If the from address is set and is not our signer, delegate to inner
        if tx.from().is_some() && tx.from() != Some(&self.address()) {
            return self
                .inner
                .send_transaction(tx, block)
                .await
                .map_err(SignerMiddlewareError::MiddlewareError);
        }

        if self.is_walletconnect && !self.is_legacy {
            let tx_hash = self
                .signer
                .send_transaction(&tx)
                .await
                .map_err(SignerMiddlewareError::SignerError)?;
            Ok(PendingTransaction::new(tx_hash, self.inner.provider()))
        } else {
            // if we have a nonce manager set, we should try handling the result in
            // case there was a nonce mismatch
            let signed_tx = self.sign_transaction(tx).await?;

            // Submit the raw transaction
            self.inner
                .send_raw_transaction(signed_tx)
                .await
                .map_err(SignerMiddlewareError::MiddlewareError)
        }
    }

    /// Signs a message with the internal signer, or if none is present it will make a call to
    /// the connected node's `eth_call` API.
    async fn sign<T: Into<Bytes> + Send + Sync>(
        &self,
        data: T,
        _: &Address,
    ) -> Result<Signature, Self::Error> {
        self.signer
            .sign_message(data.into())
            .await
            .map_err(SignerMiddlewareError::SignerError)
    }
}
