use std::ffi::OsString;
use std::fs;
use std::str::FromStr;

use ethers::abi::token::{LenientTokenizer, Token, Tokenizer};
use ethers::abi::AbiParser;
use ethers::prelude::{Middleware, SignerMiddleware};
use ethers::types::{Address, U256};

use anyhow::anyhow;
use anyhow::Context;
use regex::Regex;

use radicle_common::args::{Args, Error, Help};
use radicle_common::ethereum::{
    self,
    governance::{self, Governance},
    ProviderOptions, SignerOptions,
};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "gov",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad gov [<options>...] <command> [<args>...]
    rad gov [<options>...] execute <proposal-id>
    rad gov [<options>...] propose <proposal-file>
    rad gov [<options>...] queue <proposal-id>
    rad gov [<options>...] vote <proposal-id> (true | false)

Options

    --help  Print help

Wallet options

    --rpc-url <url>              JSON-RPC URL of Ethereum node (eg. http://localhost:8545)
    --ledger-hdpath <hdpath>     Account derivation path when using a Ledger hardware device
    --keystore <file>            Keystore file containing encrypted private key (default: none)

Commands

    execute (e)  execute a proposal
    propose (p)  make a governance proposal
    queue   (q)  queue a proposal
    vote    (v)  vote on a proposal
"#,
};

enum Command {
    Execute { id: U256 },
    Propose { file: OsString },
    Queue { id: U256 },
    Vote { id: U256 },
}

pub struct Options {
    provider: ethereum::ProviderOptions,
    signer: ethereum::SignerOptions,
    command: Command,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let parser = lexopt::Parser::from_args(args);
        let (provider, parser) = ProviderOptions::from(parser)?;
        let (signer, mut parser) = SignerOptions::from(parser)?;

        let mut command = None;

        if let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Value(val) if command.is_none() => {
                    if val == "execute" || val == "e" {
                        let id = parser
                            .value()?
                            .to_str()
                            .map(U256::from_dec_str)
                            .ok_or_else(|| anyhow!("Proposal ID is not a valid uint256"))??;
                        command = Some(Command::Execute { id });
                    } else if val == "propose" || val == "p" {
                        let file = parser.value()?;
                        command = Some(Command::Propose { file });
                    } else if val == "queue" || val == "q" {
                        let id = parser
                            .value()?
                            .to_str()
                            .map(U256::from_dec_str)
                            .ok_or_else(|| anyhow!("Proposal ID is not a valid uint256"))??;
                        command = Some(Command::Queue { id });
                    } else if val == "vote" || val == "v" {
                        let id = parser
                            .value()?
                            .to_str()
                            .map(U256::from_dec_str)
                            .ok_or_else(|| anyhow!("Proposal ID is not a valid uint256"))??;
                        command = Some(Command::Vote { id });
                    }
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        let command =
            command.ok_or_else(|| anyhow!("no command provided; see `rad gov --help`"))?;

        Ok((
            Options {
                provider,
                signer,
                command,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, _ctx: impl term::Context) -> anyhow::Result<()> {
    let rt = radicle_common::tokio::runtime::Runtime::new()?;
    let provider = ethereum::provider(options.provider)?;
    let signer_opts = options.signer;
    let (wallet, provider) = rt.block_on(term::ethereum::get_wallet(signer_opts, provider))?;
    let signer = SignerMiddleware::new(provider, wallet);
    let governance = Governance::new(signer);

    match options.command {
        Command::Execute { id } => {
            rt.block_on(run_execute(id, governance))?;
        }
        Command::Propose { file } => {
            rt.block_on(run_propose(file, governance))?;
        }
        Command::Queue { id } => {
            rt.block_on(run_queue(id, governance))?;
        }
        Command::Vote { id } => {
            rt.block_on(run_vote(id, governance))?;
        }
    }

    Ok(())
}

async fn run_execute<M>(id: U256, governance: Governance<M>) -> anyhow::Result<()>
where
    M: Middleware + 'static,
    crate::governance::Error<M>: From<<M as Middleware>::Error>,
{
    let call = governance.execute_proposal(id).await?;
    term::ethereum::transaction(call).await?;
    Ok(())
}

async fn run_propose<M>(file: OsString, governance: Governance<M>) -> anyhow::Result<()>
where
    M: Middleware + 'static,
    crate::governance::Error<M>: From<<M as Middleware>::Error>,
{
    let spinner = term::spinner(&format!(
        "Reading proposal file {}",
        term::format::highlight(&format!("{:?}", file))
    ));

    let content = fs::read_to_string(file).expect("Couldn't read proposal file.");
    let cutoff = content
        .lines()
        .position(|l| l == "## ACTIONS ##")
        .ok_or_else(|| anyhow!("Proposal is missing `### ACTIONS ###` section."))?;

    let mut targets: Vec<Address> = Vec::new();
    let mut values: Vec<U256> = Vec::new();
    let mut signatures: Vec<String> = Vec::new();
    let mut calldatas: Vec<Vec<u8>> = Vec::new();

    for l in content.lines().skip(cutoff + 1) {
        if l == "```" || l.is_empty() {
            continue;
        }
        let mut tokens = l.split(' ');

        let address = tokens
            .next()
            .context(format!("Failed to get Address in {:?}", l))?;
        let value = tokens
            .next()
            .context(format!("Failed to get Value in {:?}", l))?;
        let sig = tokens
            .next()
            .map(quoteless_string)
            .context(format!("Failed to get Function Signature in {:?}", l))?;
        let function = AbiParser::default()
            .parse_function(&sig)
            .context(format!("Failed to parse Function in {:?}", l))?;

        let args: Vec<String> = tokens.map(quoteless_string).collect();
        let args: Vec<&str> = args.iter().map(|t| t.as_str()).collect();
        let params: Vec<_> = function
            .inputs
            .iter()
            .map(|param| param.kind.clone())
            .zip(args)
            .collect();

        let tokens: anyhow::Result<Vec<Token>> = params
            .iter()
            .map(|&(ref param, value)| LenientTokenizer::tokenize(param, value))
            .collect::<Result<_, _>>()
            .map_err(From::from);
        let tokens = tokens?;

        let calldata = if !tokens.is_empty() {
            let mut calldata = function.encode_input(&tokens)?;
            calldata.drain(..4);
            calldata
        } else {
            Vec::new()
        };

        targets.push(
            Address::from_str(address)
                .context(format!("Failed to create Addresss from {}", address))?,
        );
        values.push(
            U256::from_dec_str(value).context(format!("Failed to create U256 from {}", value))?,
        );
        signatures.push(sig.to_string());
        calldatas.push(calldata.clone());
    }
    spinner.finish();

    let call = governance.propose(targets, values, signatures, calldatas, content)?;
    term::ethereum::transaction(call).await?;

    Ok(())
}

async fn run_queue<M>(id: U256, governance: Governance<M>) -> anyhow::Result<()>
where
    M: Middleware + 'static,
    crate::governance::Error<M>: From<<M as Middleware>::Error>,
{
    let call = governance.queue_proposal(id).await?;
    term::ethereum::transaction(call).await?;
    Ok(())
}

async fn run_vote<M>(id: U256, governance: Governance<M>) -> anyhow::Result<()>
where
    M: Middleware + 'static,
    crate::governance::Error<M>: From<<M as Middleware>::Error>,
{
    let proposal = governance.get_proposal(id).await?;
    let mut table = term::Table::default();
    table.push([
        term::format::bold("proposer"),
        term::format::bold("for"),
        term::format::bold("against"),
        term::format::bold("end block"),
    ]);
    table.push([
        term::format::secondary(proposal.0),
        term::format::positive(format!("▲ {}", proposal.4)),
        term::format::negative(format!("▼ {}", proposal.5)),
        term::format::secondary(proposal.3),
    ]);
    term::blank();
    table.render();
    term::blank();

    if let Some(vote) = term::select(&["approve", "reject"], &"approve") {
        let vote = *vote == "approve";
        let call = governance.cast_vote(id, vote)?;
        term::ethereum::transaction(call).await?;
    }

    Ok(())
}

fn quoteless_string(str: &str) -> String {
    let re = Regex::new(r#""?(.[^"]*)"?"#).unwrap();
    let ql = re.captures_iter(str).next().unwrap()[1].to_string();
    ql
}
