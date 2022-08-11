use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;

use ethers::prelude::{SignerMiddleware, U256};

use librad::git::identities;
use librad::git::identities::SomeIdentity::Person;
use librad::PeerId;

use radicle_common::args::{Args, Error, Help};
use radicle_common::ethereum::{
    self,
    erc_20::{Token, ERC20},
    primitives::{amount_to_u256, u256_to_amount},
    resolver::PublicResolver,
    superseeder::{Superseeder, SUPERSEEDER_ADDRESS},
    ProviderOptions, SignerOptions,
};
use radicle_common::person::Ens;
use radicle_common::{git, keys, profile};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "reward",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad reward [<range>] [<option>...]

    range defaults to 'rad/reward..' but you can use any range you want.

Options

    --dry                        Shows summary but skips signing transaction(s)
    --token                      wETH, DAI, USDC, USDT, or any other ERC-20 Address
    --amount                     Unlike interactive mode, this should be a U256 —
                                 multiple of token's smallest denomination
    --strategy                   Reward distribution strategy, can be weighted or equally
    --help                       Print help

Wallet options

    --rpc-url <url>              JSON-RPC URL of Ethereum node (eg. http://localhost:8545)
    --ledger-hdpath <hdpath>     Account derivation path when using a Ledger hardware device
    --keystore <file>            Keystore file containing encrypted private key (default: none)
    --walletconnect              Use WalletConnect

Environment variables

    ETH_RPC_URL  Ethereum JSON-RPC URL (overwrite with '--rpc-url')
    ETH_HDPATH   Hardware wallet derivation path (overwrite with '--ledger-hdpath')
"#,
};

#[derive(Debug, Eq, PartialEq)]
pub enum Strategy {
    Equally,
    Weighted,
}

impl FromStr for Strategy {
    type Err = anyhow::Error;

    fn from_str(strategy: &str) -> Result<Strategy, Self::Err> {
        match strategy.to_lowercase().as_str() {
            "equally" => Ok(Strategy::Equally),
            "weighted" => Ok(Strategy::Weighted),
            _ => Err(anyhow::anyhow!("Strategy undefined: {}", strategy)),
        }
    }
}

#[derive(Debug)]
pub struct Options {
    pub range: Option<String>,
    pub dry: bool,
    pub amount: Option<U256>,
    pub token: Option<Token>,
    pub strategy: Option<Strategy>,
    pub provider: ethereum::ProviderOptions,
    pub signer: ethereum::SignerOptions,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let parser = lexopt::Parser::from_args(args);
        let (provider, parser) = ProviderOptions::from(parser)?;
        let (signer, mut parser) = SignerOptions::from(parser)?;
        let mut range = None;
        let mut dry = false;
        let mut amount = None;
        let mut token = None;
        let mut strategy = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("dry") => {
                    dry = true;
                }
                Value(val) if range.is_none() => {
                    range = val.into_string().ok();
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("token") => {
                    token = parser.value()?.parse().ok();
                }
                Long("amount") => {
                    let amt = parser
                        .value()?
                        .into_string()
                        .map_err(|_| anyhow!("Can't parse --amount value"))?;
                    amount = U256::from_dec_str(&amt).ok();
                }
                Long("strategy") => {
                    strategy = parser.value()?.parse().ok();
                }
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                range,
                dry,
                amount,
                token,
                strategy,
                provider,
                signer,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let rt = radicle_common::tokio::runtime::Runtime::new()?;
    let provider = ethereum::provider(options.provider)?;
    let signer_opts = options.signer;
    let (wallet, provider) = rt.block_on(term::ethereum::get_wallet(signer_opts, provider))?;
    let signer: Arc<_> = SignerMiddleware::new(provider, wallet).into();
    let profile = ctx.profile()?;
    let storage = profile::read_only(&profile)?;
    let repo = git::Repository::open(Path::new("."))?;

    let mut revwalk = repo.revwalk()?;
    let range = options
        .range
        .or_else(|| {
            if !repo.tag_names(Some("rad/reward")).ok()?.is_empty() {
                return Some("rad/reward..".to_string());
            }
            None
        })
        .ok_or_else(|| anyhow!("Can't determine the range of commits"))?;
    revwalk.push_range(&range)?;

    // from identities in local monorepo build a map of: fp -> ens
    let mut ssh_to_ens: HashMap<String, _> = HashMap::new();
    for identity in identities::any::list(&storage)?.flatten() {
        if let Person(person) = identity {
            let peer_ids = person
                .delegations()
                .iter()
                .map(|key| -> PeerId { key.to_owned().into() })
                .collect::<Vec<PeerId>>();
            if let Ok(Some(ens)) = person.payload().get_ext::<Ens>() {
                for peer_id in peer_ids {
                    let fp = keys::to_ssh_fingerprint(&peer_id)?;
                    ssh_to_ens.insert(fp.to_string(), ens.clone());
                }
            }
        }
    }

    term::headline("Retrieving commits...");

    // collect data regarding provided range
    let range_data = revwalk
        .filter_map(|sha1| {
            let sha1 = match sha1 {
                Err(_) => return None,
                Ok(sha1) => sha1,
            };
            let fp = git::commit_ssh_fingerprint(Path::new("."), &sha1.to_string()).ok()?;
            let ens = match fp {
                Some(ref fp) => ssh_to_ens.get(fp),
                None => None,
            };
            Some((sha1, fp, ens))
        })
        .collect::<Vec<_>>();

    let all_ens = range_data
        .iter()
        .filter_map(|(_, _, ens)| *ens)
        .collect::<Vec<_>>();

    let head_sha1 = range_data.first().map(|(sha1, _, _)| *sha1);

    // show commits in the provided range
    let mut table = term::Table::default();
    table.push([
        term::format::bold("Commit"),
        term::format::bold("Signature"),
        term::format::bold("ENS"),
    ]);

    for (sha1, fp, ens) in range_data {
        match fp {
            None => {
                table.push([
                    term::format::highlight(sha1),
                    term::format::tertiary("missing"),
                    term::format::secondary("missing"),
                ]);
            }
            Some(fp) => {
                table.push([
                    term::format::highlight(sha1),
                    term::format::tertiary(fp.clone()),
                    term::format::secondary(
                        ens.map(|ens| ens.name.to_owned())
                            .unwrap_or_else(|| String::from("missing")),
                    ),
                ]);
            }
        }
    }

    table.render();
    term::blank();

    if all_ens.is_empty() {
        return Err(anyhow!(
            "No contributor with a set ENS was found in the given range"
        ));
    }

    let token: Token = options
        .token
        .or_else(|| {
            term::select_with_prompt(
                "What's the reward's token?",
                &["wETH", "DAI", "USDC", "USDT"],
                &"wETH",
            )
            .unwrap()
            .parse()
            .ok()
        })
        .ok_or_else(|| anyhow!("Couldn't determine reward's token"))?;

    let address = token.get_address();

    let decimals = token
        .get_decimals()
        .or_else(|| {
            let token: ERC20<SignerMiddleware<_, _>> = ERC20::new(signer.clone(), address);
            let symbol = rt.block_on(token.get_symbol()).ok()?;

            term::blank();
            term::info!(
                "You have selected {} token",
                term::format::highlight(term::format::bold(symbol))
            );
            term::blank();

            let decimals = rt.block_on(token.get_decimals()).ok()?.into();
            Some(decimals)
        })
        .ok_or_else(|| anyhow!("Couldn't determine token's decimals"))?;

    let amount = options
        .amount
        .or_else(|| {
            let mut amount = U256::from(0_u64);
            while amount.is_zero() {
                let amt: String = term::text_input("Total reward amount?", None).unwrap();
                amount = amount_to_u256(&amt, decimals).ok()??;
                if amount.is_zero() {
                    term::warning("Reward amount cannot be zero");
                }
            }
            Some(amount)
        })
        .ok_or_else(|| anyhow!("Couldn't parse amount as u256"))?;

    let strategy = options
        .strategy
        .or_else(|| {
            term::select_with_prompt(
                "How should the reward be distributed?",
                &["Equally", "Weighted"],
                &"Equally",
            )
            .unwrap()
            .parse()
            .ok()
        })
        .ok_or_else(|| anyhow!("Couldn't determine distribution strategy"))?;

    let rewards = calculate_rewards(strategy, amount, &all_ens)?;

    // show summary and aggregate payments
    term::blank();
    let mut table = term::Table::default();
    table.push([
        term::format::bold("Address"),
        term::format::bold("Reward"),
        term::format::bold("ENS"),
    ]);

    let mut receivers = Vec::new();
    let mut amounts = Vec::new();

    for (ens, reward) in rewards.iter() {
        let resolver: Result<PublicResolver<SignerMiddleware<_, _>>, _> =
            rt.block_on(PublicResolver::get(ens, signer.clone()));

        // if resolver doesn't exist, we just skip this one
        if resolver.is_err() {
            table.push([
                term::format::italic(term::format::negative("Missing")),
                term::format::tertiary(u256_to_amount(*reward, decimals)?),
                term::format::secondary(ens),
            ]);
            continue;
        }
        let resolver = resolver.unwrap();

        let address = rt
            .block_on(resolver.address(ens))?
            .ok_or_else(|| anyhow!("Couldn't get Address of ENS"))?;

        receivers.push(address);
        amounts.push(*reward);

        table.push([
            term::format::highlight(address),
            term::format::tertiary(u256_to_amount(*reward, decimals)?),
            term::format::secondary(ens),
        ]);
    }

    table.render();
    term::blank();

    // exit now if this was a dry run
    if options.dry {
        return Ok(());
    }

    if term::confirm("Do you wish to proceed?") {
        let sum = amounts
            .iter()
            .fold(Some(U256::from(0_u64)), |sum, amt| {
                sum.and_then(|s| s.checked_add(*amt))
            })
            .ok_or_else(|| anyhow!("Failed to sum all transactions amounts"))?;

        let token: ERC20<SignerMiddleware<_, _>> = ERC20::new(signer.clone(), address);
        let spinner = term::spinner(&term::format::tertiary("Checking allowance..."));
        let allowance = rt.block_on(token.get_allowance(signer.address(), *SUPERSEEDER_ADDRESS))?;
        spinner.finish();

        if allowance < sum {
            term::blank();
            term::info!(
                "{}",
                term::format::tertiary("Approving transaction amount for Superseeder...")
            );
            term::blank();

            let call = token.approve(*SUPERSEEDER_ADDRESS, sum)?;
            rt.block_on(term::ethereum::transaction(call))?;
        }

        // generate tx(s)
        let superseeder: Superseeder<SignerMiddleware<_, _>> = Superseeder::new(signer);
        let call = superseeder.send(address, receivers, amounts)?;

        term::blank();
        term::info!("{}", term::format::tertiary("Sending transaction..."));
        term::blank();

        rt.block_on(term::ethereum::transaction(call))?;

        // tag last processed commit
        if let Some(head_sha1) = head_sha1 {
            let spinner = term::spinner(&term::format::tertiary(
                "Tagging the tip of range as `rad/reward`",
            ));
            let head_commit = repo.find_commit(head_sha1)?.into_object();
            repo.tag_lightweight("rad/reward", &head_commit, true)?;
            spinner.finish();
        }
    }

    Ok(())
}

fn calculate_rewards(
    strategy: Strategy,
    reward: U256,
    all_ens: &[&Ens],
) -> anyhow::Result<HashMap<String, U256>> {
    let mut shares: HashMap<String, usize> = HashMap::new();
    for ens in all_ens {
        if let Some(v) = shares.get_mut(&ens.name) {
            *v += 1;
        } else {
            shares.insert(ens.name.clone(), 1);
        }
    }

    let mut rewards: HashMap<String, U256> = HashMap::new();
    if strategy == Strategy::Equally {
        let count = U256::from(shares.len());
        let each_reward = reward
            .checked_div(count)
            .ok_or_else(|| anyhow!("Couldn't divide total reward by number of contributors"))?;
        for ens in shares.keys() {
            rewards.insert(ens.to_string(), each_reward);
        }
    } else if strategy == Strategy::Weighted {
        let total_shares = shares.values().sum::<usize>();
        let reward_per_share = reward
            .checked_div(U256::from(total_shares))
            .ok_or_else(|| anyhow!("Failed to divide total reward by total number of shares"))?;
        for (ens, share) in shares {
            let this_reward = reward_per_share
                .checked_mul(U256::from(share))
                .ok_or_else(|| {
                    anyhow!("Failed to multiply reward by number of shares for {}", ens)
                })?;
            rewards.insert(ens.to_string(), this_reward);
        }
    }

    Ok(rewards)
}
