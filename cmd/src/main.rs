use std::time::UNIX_EPOCH;
use std::{path, rc::Rc, str::FromStr};

use anchor_client::anchor_lang::system_program;
use anchor_client::solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use anchor_client::solana_client::rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};
use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_client::{solana_sdk::commitment_config::CommitmentConfig, Client, Cluster};
use clap::{Parser, Subcommand};
use home::home_dir;
use sha2::{Digest, Sha256};
use solana_account_decoder::UiDataSliceConfig;
use solana_sdk::bs58;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::signature::read_keypair_file;
use solana_sdk::signer::Signer;

use bottle::instruction as BottleInstruction;
use bottle::{
    accounts as BottleAccounts, Bottle, BAG_SEED, BOTTLE_ASSET_SEED, BOTTLE_SEED, RETRIEVE_SEED,
    THROW_SEED,
};

/// Simple client to interact with the `Bottle` program which is on chain
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Sepcify the wallet file
    #[arg(short, long, default_value = None)]
    wallet: Option<String>,

    /// Cluster to connect to
    #[arg(short, long, default_value = "l")]
    cluster: String,

    /// Program ID
    #[arg(short, long)]
    program_id: Pubkey,

    #[command(subcommand)]
    subcmd: Subcmd,
}

#[derive(Subcommand)]
enum Subcmd {
    /// Throw some message at the bottle to the chain
    Throw {
        /// The message to write at the bottle
        #[arg(short, long)]
        message: String,
        /// The amount of SOL to throw at the bottle
        #[arg(short, long, default_value_t = 0)]
        amount: u64,
    },
    /// Retrieve the message from a bottle
    Retrieve,
}

fn calculate_discriminator(input: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash: [u8; 32] = hasher.finalize().into();
    hash[..8].try_into().unwrap()
}

fn main() {
    let args = Args::parse();
    let wallet = args.wallet;
    let cluster = args.cluster;
    let program_id = args.program_id;
    let wallet_path = path::PathBuf::from(
        wallet.unwrap_or(
            home_dir()
                .unwrap()
                .join(".config/solana/id.json")
                .to_str()
                .unwrap_or_default()
                .to_string(),
        ),
    );

    let payer = read_keypair_file(wallet_path).expect("Cannot get the wallet file");
    let cluster = Cluster::from_str(&cluster).expect("Wrong Cluster. Olny supports 't|testnet', 'm|mainnet', 'd|devnet', 'l|localnet', 'g|debug' or custom url");
    let payer = Rc::new(payer);
    let client = Client::new_with_options(
        cluster.clone(),
        payer.clone(),
        CommitmentConfig::processed(),
    );
    let program = client.program(program_id).expect("Cannot get the program");

    match args.subcmd {
        Subcmd::Throw { message, amount } => {
            let now = std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Wrong time")
                .as_millis() as u64;
            let (bottle, _) = Pubkey::find_program_address(
                &[
                    BOTTLE_SEED.as_bytes(),
                    payer.pubkey().as_ref(),
                    now.to_le_bytes().as_ref(),
                ],
                &program_id,
            );
            let (bottle_asset, _) = Pubkey::find_program_address(
                &[
                    BOTTLE_ASSET_SEED.as_bytes(),
                    payer.pubkey().as_ref(),
                    now.to_le_bytes().as_ref(),
                ],
                &program_id,
            );
            let (bag, _) = Pubkey::find_program_address(
                &[
                    BAG_SEED.as_bytes(),
                    payer.pubkey().as_ref(),
                    THROW_SEED.as_bytes(),
                ],
                &program_id,
            );

            let tx = program
                .request()
                .accounts(BottleAccounts::ThrowABottle {
                    sender: payer.pubkey(),
                    bottle_asset,
                    bottle,
                    bag,
                    system_program: system_program::ID,
                })
                .args(BottleInstruction::ThrowABottle {
                    id: now,
                    asset: amount,
                    message,
                })
                .send()
                .expect("Throw a bottle failed");

            println!(
                "😊 You just throw a bottle to the chain, the transaction's signature is: {:?}",
                tx
            );
        }
        Subcmd::Retrieve => {
            // pre-fetch the drifting bottles
            let bottle_discriminator = calculate_discriminator("account:Bottle");
            let discriminator_memcmp = RpcFilterType::Memcmp(Memcmp::new(
                0,                                                                            // offset
                MemcmpEncodedBytes::Base58(bs58::encode(bottle_discriminator).into_string()), // encoded bytes
            ));
            // only fetch drfting bottles
            let drifting_memcmp = RpcFilterType::Memcmp(Memcmp::new(
                56,                                                          // offset
                MemcmpEncodedBytes::Base58(bs58::encode([0]).into_string()), // encoded bytes
            ));
            let config = RpcProgramAccountsConfig {
                filters: Some(vec![discriminator_memcmp, drifting_memcmp]),
                account_config: RpcAccountInfoConfig {
                    // get id
                    data_slice: Some(UiDataSliceConfig {
                        offset: 8,
                        length: 8,
                    }),
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut accounts = program
                .rpc()
                .get_program_accounts_with_config(&program_id, config)
                .expect("There is no drifting bottles");
            accounts.sort_by_key(|(_, account)| {
                u64::from_le_bytes(account.data[0..8].try_into().unwrap())
            });

            if accounts.is_empty() {
                println!("There is no drifting bottles");
                return;
            }

            // get the 1st drifting bottle
            let drifting_bottle_pubkey = accounts[0].0;
            let drifting_bottle: Bottle = program
                .account(drifting_bottle_pubkey)
                .expect("Couldn't get the drifting bottle account");

            let (bag, _) = Pubkey::find_program_address(
                &[
                    BAG_SEED.as_bytes(),
                    payer.pubkey().as_ref(),
                    RETRIEVE_SEED.as_bytes(),
                ],
                &program_id,
            );
            program
                .request()
                .accounts(BottleAccounts::RetrieveABottle {
                    bottle: drifting_bottle_pubkey,
                    bottle_asset: drifting_bottle.asset_account,
                    retrievee: payer.pubkey(),
                    bag,
                    system_program: system_program::ID,
                })
                .args(BottleInstruction::RetrieveABottle {})
                .send()
                .expect("Retrieve a bottle failed");
            let bottle_account: Bottle = program
                .account(drifting_bottle_pubkey)
                .expect("Cannot get the bag account");
            println!(
                "😎 You just received a bottle that says: {:?}, and {} SOL",
                bottle_account.message,
                bottle_account.asset / LAMPORTS_PER_SOL
            );
        }
    }
}
