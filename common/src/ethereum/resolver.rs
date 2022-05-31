use std::sync::Arc;

use ethers::prelude::{signer::SignerMiddlewareError, Http, Middleware, ProviderError};
use ethers::types::{Address, Bytes};
use ethers::{
    abi::{Abi, Detokenize, ParamType},
    contract::{AbiError, Contract, ContractError},
    prelude::builders::ContractCall,
    providers::{ens::ENS_ADDRESS, Provider},
};

use crate::ethereum;

pub const RADICLE_ID_KEY: &str = "eth.radicle.id";
pub const RADICLE_SEED_ID_KEY: &str = "eth.radicle.seed.id";
pub const RADICLE_SEED_HOST_KEY: &str = "eth.radicle.seed.host";

const PUBLIC_RESOLVER_ABI: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/abis/PublicResolver.json"
));

pub struct PublicResolver<M> {
    contract: Contract<M>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error<M: Middleware> {
    #[error("ENS name '{name}' not found")]
    NameNotFound { name: String },
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Contract(#[from] ContractError<M>),
    #[error(transparent)]
    Abi(#[from] ethers::abi::Error),
    #[error(transparent)]
    SignerMiddleware(#[from] SignerMiddlewareError<Provider<Http>, ethereum::Wallet>),
}

impl<M> PublicResolver<M>
where
    M: Middleware,
    Error<M>: From<<M as Middleware>::Error>,
{
    pub fn new(addr: Address, client: impl Into<Arc<M>>) -> Self {
        let abi: Abi = serde_json::from_str(PUBLIC_RESOLVER_ABI).expect("The ABI is valid");
        let contract = Contract::new(addr, abi, client);

        Self { contract }
    }

    pub async fn get(name: &str, client: impl Into<Arc<M>>) -> Result<Self, Error<M>> {
        let client = client.into();
        let bytes = client
            .call(
                &ethers::providers::ens::get_resolver(ENS_ADDRESS, name).into(),
                None,
            )
            .await?;
        let tokens = ethers::abi::decode(&[ParamType::Address], bytes.as_ref())?;
        let resolver = Address::from_tokens(tokens).unwrap();

        if resolver == Address::zero() {
            return Err(Error::NameNotFound {
                name: name.to_owned(),
            });
        }
        Ok(Self::new(resolver, client))
    }

    pub fn multicall(&self, calls: Vec<Bytes>) -> Result<ContractCall<M, Vec<Bytes>>, AbiError> {
        self.contract.method("multicall", calls)
    }

    pub async fn text(&self, name: &str, key: &str) -> Result<Option<String>, Error<M>> {
        let node = ethers::providers::ens::namehash(name);
        let value: String = self
            .contract
            .method("text", (node, key.to_owned()))
            .map_err(ContractError::from)?
            .call()
            .await?;

        if value.is_empty() {
            return Ok(None);
        }
        Ok(Some(value))
    }

    pub async fn address(&self, name: &str) -> Result<Option<Address>, Error<M>> {
        let node = ethers::providers::ens::namehash(name);
        let addr: Address = self
            .contract
            .method("addr", node)
            .map_err(ContractError::from)?
            .call()
            .await?;

        if addr.is_zero() {
            return Ok(None);
        }
        Ok(Some(addr))
    }

    pub fn set_address(&self, name: &str, addr: Address) -> Result<ContractCall<M, ()>, AbiError> {
        let node = ethers::providers::ens::namehash(name);

        self.contract.method("setAddr", (node, addr))
    }

    pub fn set_text(
        &self,
        name: &str,
        key: &str,
        val: &str,
    ) -> Result<ContractCall<M, ()>, AbiError> {
        let node = ethers::providers::ens::namehash(name);

        self.contract
            .method("setText", (node, key.to_owned(), val.to_owned()))
    }
}
