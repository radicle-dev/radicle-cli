use std::str::FromStr;
use std::sync::Arc;

use ethers::prelude::builders::ContractCall;
use ethers::prelude::{signer::SignerMiddlewareError, Http, Lazy, Middleware, ProviderError};
use ethers::types::{Address, U256};
use ethers::{
    abi::Abi,
    contract::{AbiError, Contract, ContractError},
    providers::Provider,
};

use crate::ethereum;

const ABI: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/abis/ERC-20.json"));

static WETH_ADDRESS: Lazy<Address> =
    Lazy::new(|| Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap());
static DAI_ADDRESS: Lazy<Address> =
    Lazy::new(|| Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f").unwrap());
static USDC_ADDRESS: Lazy<Address> =
    Lazy::new(|| Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap());
static USDT_ADDRESS: Lazy<Address> =
    Lazy::new(|| Address::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7").unwrap());

pub struct ERC20<M> {
    contract: Contract<M>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error<M: Middleware> {
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Contract(#[from] ContractError<M>),
    #[error(transparent)]
    Abi(#[from] ethers::abi::Error),
    #[error(transparent)]
    ContractAbi(#[from] AbiError),
    #[error(transparent)]
    SignerMiddleware(#[from] SignerMiddlewareError<Provider<Http>, ethereum::Wallet>),
}

impl<M> ERC20<M>
where
    M: Middleware,
    Error<M>: From<<M as Middleware>::Error>,
{
    pub fn new(client: impl Into<Arc<M>>, address: Address) -> Self {
        let abi: Abi = serde_json::from_str(ABI).expect("The ABI is valid");
        let contract = Contract::new(address, abi, client);

        Self { contract }
    }

    pub async fn get_symbol(&self) -> Result<String, Error<M>> {
        let symbol = self
            .contract
            .method("symbol", ())
            .map_err(ContractError::from)?
            .call()
            .await?;

        Ok(symbol)
    }

    pub async fn get_decimals(&self) -> Result<u8, Error<M>> {
        let symbol = self
            .contract
            .method("decimals", ())
            .map_err(ContractError::from)?
            .call()
            .await?;

        Ok(symbol)
    }

    pub async fn get_allowance(&self, owner: Address, spender: Address) -> Result<U256, Error<M>> {
        let allowance = self
            .contract
            .method("allowance", (owner, spender))
            .map_err(ContractError::from)?
            .call()
            .await?;

        Ok(allowance)
    }

    pub fn approve(&self, spender: Address, amount: U256) -> Result<ContractCall<M, ()>, Error<M>> {
        self.contract
            .method("approve", (spender, amount))
            .map_err(Error::ContractAbi)
    }
}

#[derive(Debug)]
pub enum Token {
    WETH,
    DAI,
    USDC,
    USDT,
    Other(Address), // e.g 0x2b591e99afe9f32eaa6214f7b7629768c40eeb39
}

impl FromStr for Token {
    type Err = anyhow::Error;

    fn from_str(token: &str) -> Result<Token, Self::Err> {
        match token.to_uppercase().as_str() {
            "WETH" => Ok(Token::WETH),
            "DAI" => Ok(Token::DAI),
            "USDC" => Ok(Token::USDC),
            "USDT" => Ok(Token::USDT),
            _ => {
                if let Ok(address) = Address::from_str(token) {
                    Ok(Token::Other(address))
                } else {
                    Err(anyhow::anyhow!(
                        "Failed to parse token's address: {}",
                        token
                    ))
                }
            }
        }
    }
}

impl Token {
    pub fn get_decimals(&self) -> Option<usize> {
        match self {
            Token::WETH => Some(18),
            Token::DAI => Some(18),
            Token::USDC => Some(6),
            Token::USDT => Some(6),
            Token::Other(_) => None,
        }
    }

    pub fn get_address(&self) -> Address {
        match self {
            Token::WETH => *WETH_ADDRESS,
            Token::DAI => *DAI_ADDRESS,
            Token::USDC => *USDC_ADDRESS,
            Token::USDT => *USDT_ADDRESS,
            Token::Other(address) => *address,
        }
    }
}
