use ethers::abi::Detokenize;
use ethers::prelude::signer::SignerMiddlewareError;
use ethers::prelude::*;
use ethers::types::transaction::eip2718::TypedTransaction;

use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;

/// Trait for sending transactions
///
/// Implement this trait to support WalletConnect in Leagcy and EIP-1559 mode.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait ExtendedSigner: Debug + Send + Sync + Signer {
    /// Sends the transaction (only applicable for walletconnect)
    async fn send_transaction(&self, message: &TypedTransaction) -> Result<H256, Self::Error>;

    /// Check if signer is a WalletConnect signer
    fn is_walletconnect(&self) -> bool;

    /// Check if signer is a legacy signer
    fn is_legacy(&self) -> bool;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait ExtendedMiddleware: Sync + Send + Debug + Middleware {
    async fn send_transaction<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        tx: T,
        block: Option<BlockId>,
    ) -> Result<PendingTransaction<'_, Self::Provider>, Self::Error>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<M, S> ExtendedMiddleware for SignerMiddleware<M, S>
where
    M: Middleware,
    S: ExtendedSigner,
{
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
                .inner()
                .send_transaction(tx, block)
                .await
                .map_err(SignerMiddlewareError::MiddlewareError);
        }

        if self.signer().is_walletconnect() && !self.signer().is_legacy() {
            let tx_hash = self
                .signer()
                .send_transaction(&tx)
                .await
                .map_err(SignerMiddlewareError::SignerError)?;
            Ok(PendingTransaction::new(tx_hash, self.inner().provider()))
        } else {
            let signature = self
                .signer()
                .sign_transaction(&tx)
                .await
                .map_err(SignerMiddlewareError::SignerError)?;

            // if we have a nonce manager set, we should try handling the result in
            // case there was a nonce mismatch
            //
            // Return the raw rlp-encoded signed transaction
            let signed_tx = tx.rlp_signed(self.signer().chain_id(), &signature);

            // Submit the raw transaction
            self.inner()
                .send_raw_transaction(signed_tx)
                .await
                .map_err(SignerMiddlewareError::MiddlewareError)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContractCall<M, D> {
    pub inner: ethers::prelude::builders::ContractCall<M, D>,
    pub client: Arc<M>,
    pub legacy: bool,
}

impl<M, D> ContractCall<M, D>
where
    M: ExtendedMiddleware,
    D: Detokenize,
{
    /// Sets the type of transaction which is either Legacy or EIP-1559
    pub fn set_tx_type(mut self) -> Self {
        if self.legacy {
            self.inner = self.inner.legacy();
        }

        self
    }

    /// Returns the underlying transaction's ABI encoded data
    pub fn calldata(&self) -> Option<Bytes> {
        self.inner.tx.data().cloned()
    }

    /// Returns the estimated gas cost for the underlying transaction to be executed
    pub async fn estimate_gas(&self) -> Result<U256, ContractError<M>> {
        self.client
            .estimate_gas(&self.inner.tx)
            .await
            .map_err(ContractError::MiddlewareError)
    }

    /// Queries the blockchain via an `eth_call` for the provided transaction.
    ///
    /// If executed on a non-state mutating smart contract function (i.e. `view`, `pure`)
    /// then it will return the raw data from the chain.
    ///
    /// If executed on a mutating smart contract function, it will do a "dry run" of the call
    /// and return the return type of the transaction without mutating the state
    ///
    /// Note: this function _does not_ send a transaction from your account
    pub async fn call(&self) -> Result<D, ContractError<M>> {
        let bytes = self
            .client
            .call(&self.inner.tx, self.inner.block)
            .await
            .map_err(ContractError::MiddlewareError)?;

        // decode output
        let data = decode_function_data(&self.inner.function, &bytes, false)?;

        Ok(data)
    }

    /// Signs and broadcasts the provided transaction
    pub async fn send(&self) -> Result<PendingTransaction<'_, M::Provider>, ContractError<M>> {
        ExtendedMiddleware::send_transaction(&*self.client, self.inner.tx.clone(), self.inner.block)
            .await
            .map_err(ContractError::MiddlewareError)
    }
}
