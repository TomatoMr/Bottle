use anchor_lang::prelude::*;

declare_id!("G36SiH1Cp3kyFazCwTd18t753JHchYKyxEriwQvZMead");

pub const BOTTLE_SEED: &str = "bottle";
pub const BOTTLE_ASSET_SEED: &str = "bottle_asset";
pub const BAG_SEED: &str = "bag";
pub const THROW_SEED: &str = "throw";
pub const RETRIEVE_SEED: &str = "retrieve";

#[program]
pub mod bottle {
    use anchor_lang::{
        solana_program::{clock::SECONDS_PER_DAY, native_token::LAMPORTS_PER_SOL},
        system_program,
    };

    use super::*;

    pub fn throw_a_bottle(
        ctx: Context<ThrowABottle>,
        id: u64,
        asset: u64,
        message: String,
    ) -> Result<()> {
        let sender = &ctx.accounts.sender;
        let bottle_asset = &mut ctx.accounts.bottle_asset;
        let bottle = &mut ctx.accounts.bottle;
        let bag = &mut ctx.accounts.bag;
        let clock = Clock::get()?;
        let now = clock.unix_timestamp;

        if message.len() > MAX_MESSAGE_SIZE {
            return Err(BottleError::MessageTooLong.into());
        }

        if bag.counter >= Bag::MAX_BOTTLES_PER_DAY {
            if now - bag.last_bottle_time <= SECONDS_PER_DAY as i64 {
                return Err(BagError::MaxDailyBottleExceeded.into());
            } else {
                bag.counter = 0;
            }
        }

        if asset > 0 {
            let cpi_ctx = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: sender.to_account_info().clone(),
                    to: bottle_asset.clone(),
                },
            );
            system_program::transfer(cpi_ctx, asset * LAMPORTS_PER_SOL)?;
            bottle.asset = asset * LAMPORTS_PER_SOL;
            bottle.asset_account = *bottle_asset.key;
        }

        bottle.id = id;
        bottle.bump = ctx.bumps.bottle_asset;
        bottle.sender = *sender.key;
        bottle.timestamp = now;
        bottle.state = BottleState::Drifting;
        bottle.message = message;

        bag.counter += 1;
        bag.last_bottle_time = now;

        Ok(())
    }

    pub fn retrieve_a_bottle(ctx: Context<RetrieveABottle>) -> Result<()> {
        let bottle = &mut ctx.accounts.bottle;
        let bottle_asset = &mut ctx.accounts.bottle_asset;
        let retrievee = &mut ctx.accounts.retrievee;
        let bag = &mut ctx.accounts.bag;
        let clock = Clock::get()?;
        let now = clock.unix_timestamp;

        if bottle.sender.key() == retrievee.key.key() {
            return Err(BagError::CannotRetrieveOwnBottle.into());
        }

        if bag.counter >= Bag::MAX_BOTTLES_PER_DAY {
            if now - bag.last_bottle_time <= SECONDS_PER_DAY as i64 {
                return Err(BagError::MaxDailyBottleExceeded.into());
            } else {
                bag.counter = 0;
            }
        }

        if !matches!(bottle.state, BottleState::Drifting) {
            return Err(BagError::BottleAlreadyRetrieved.into());
        }

        if bottle.asset > 0 {
            let id = bottle.id.to_le_bytes();
            let seeds: &[&[&[u8]]; 1] = &[&[
                b"bottle_asset",
                bottle.sender.as_ref(),
                id.as_ref(),
                &[bottle.bump],
            ]];
            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: bottle_asset.to_account_info().clone(),
                    to: retrievee.to_account_info().clone(),
                },
                seeds,
            );

            system_program::transfer(cpi_ctx, bottle.asset)?;
        }

        bottle.state = BottleState::Retrieved;

        bag.counter += 1;
        bag.last_bottle_time = now;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct ThrowABottle<'info> {
    /// Who pays for the bottle.
    #[account(mut)]
    pub sender: Signer<'info>,
    /// Who holds the assets in the bottle.
    /// CHECK: This is not dangaerous, beacuse it will be checked in the instruction.
    #[account(
        mut,
        seeds = [BOTTLE_ASSET_SEED.as_bytes(), sender.key.as_ref(), id.to_le_bytes().as_ref()], 
        bump
    )]
    pub bottle_asset: AccountInfo<'info>,
    /// The bottle account that contains the data of a bottle.
    #[account(
        init,
        payer = sender,
        space = Bottle::SPACE,
        seeds = [BOTTLE_SEED.as_bytes(), sender.key.as_ref(), id.to_le_bytes().as_ref()], 
        bump
    )]
    pub bottle: Account<'info, Bottle>,
    /// Bag for storing bottles to be throwed.
    #[account(
            init_if_needed,
            payer = sender,
            space = Bag::SPACE,
            seeds = [BAG_SEED.as_bytes(), sender.key.as_ref(), THROW_SEED.as_bytes()], 
            bump
        )]
    pub bag: Account<'info, Bag>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RetrieveABottle<'info> {
    /// The bottle account that contains the data of a bottle.
    #[account(mut)]
    pub bottle: Account<'info, Bottle>,
    /// Who holds the assets in the bottle.
    /// CHECK: This is not dangaerous, beacuse it will be checked in the instruction.
    #[account(mut, address = bottle.asset_account)]
    pub bottle_asset: AccountInfo<'info>,
    /// Who retrieves the bottle.
    #[account(mut)]
    pub retrievee: Signer<'info>,
    /// Bag for storing retrieved bottles.
    #[account(
        init_if_needed,
        payer = retrievee,
        space = Bag::SPACE,
        seeds = [BAG_SEED.as_bytes(), retrievee.key.as_ref(), RETRIEVE_SEED.as_bytes()], 
        bump
    )]
    pub bag: Account<'info, Bag>,
    pub system_program: Program<'info, System>,
}

const DISCRIMINATOR_SIZE: usize = 8;
const ID_SIZE: usize = 8;
const PUBLIC_KEY_SIZE: usize = 32;
const TIMESTAMP_SIZE: usize = 8;
const STATE_SIZE: usize = 1;
const BUMP_SIZE: usize = 1;
const STRING_PREFIX_SIZE: usize = 4; // Stores the size of the string.
const MAX_MESSAGE_SIZE: usize = 400;

#[derive(Debug)]
#[account]
pub struct Bottle {
    /// A unique ID for each bottle.
    pub id: u64,
    /// Who drifts the bottle.
    pub sender: Pubkey,
    /// Time to throw the bottle.
    pub timestamp: i64,
    /// Current state of the bottle. Only "Drifting" bottle can be retrieved.
    pub state: BottleState,
    /// The assets that the sender puts into the bottle.
    pub asset: u64,
    /// The account that holds the assets.
    pub asset_account: Pubkey,
    /// The bump of the account.
    pub bump: u8,
    /// Message to be displayed on the bottle.
    pub message: String,
}

impl Bottle {
    const SPACE: usize = DISCRIMINATOR_SIZE
        + ID_SIZE
        + PUBLIC_KEY_SIZE
        + TIMESTAMP_SIZE
        + STATE_SIZE
        + BUMP_SIZE
        + STRING_PREFIX_SIZE
        + MAX_MESSAGE_SIZE;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum BottleState {
    /// Drifting: The bottle is currently Drifting in the water.
    Drifting,
    /// Retrieved: The bottle has been retrieved from the water.
    Retrieved,
}

#[error_code]
pub enum BottleError {
    #[msg("The message is too long.")]
    MessageTooLong,
}

#[derive(Debug)]
/// The bag stroes bottles.
#[account]
pub struct Bag {
    /// Last time a bottle was throwed or retrieved.
    pub last_bottle_time: i64,
    /// How many bottles have been throwed or retrieved.
    pub counter: u8,
}

const COUNTER_SIZE: usize = 1;
impl Bag {
    const MAX_BOTTLES_PER_DAY: u8 = 3;
    const SPACE: usize = DISCRIMINATOR_SIZE + TIMESTAMP_SIZE + COUNTER_SIZE;
}

#[error_code]
pub enum BagError {
    #[msg("The maximum number of bottles that can be throwed or retrieved each day has exceeded the limit.")]
    MaxDailyBottleExceeded,
    #[msg("This bottle has already been retrieved.")]
    BottleAlreadyRetrieved,
    #[msg("The same person cannot retrieve their own bottle.")]
    CannotRetrieveOwnBottle,
}
