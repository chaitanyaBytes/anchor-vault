#![allow(unexpected_cfgs)]
use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};

declare_id!("4jbEPodLWz37hBLwHoETuEEK6jdmEkTT4q3iFcqHhn6B");

#[program]
pub mod vault {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.initialize(&ctx.bumps)
    }

    pub fn deposit(ctx: Context<Payment>, amount: u64) -> Result<()> {
        ctx.accounts.deposit(amount)
    }

    pub fn withdraw(ctx: Context<Payment>, amount: u64) -> Result<()> {
        ctx.accounts.withdraw(amount)
    }

    pub fn close(ctx: Context<CloseAccounts>) -> Result<()> {
        ctx.accounts.close()
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    // 'info telling that all the references inside the sruct will live as long as the struct itself
    // Everything in this struct only lives while the instruction is executing.
    // If I use user somewhere where the account it refers to is not valid, that would be a dangling pointer, and 'info helps ensure that user lives only as long as the struct it's in.
    #[account(mut)]
    // this is the account that is responsible for paying the rent of any accounts created in this ix
    // and also signing the transactions
    pub user: Signer<'info>,

    #[account(
        init,
        payer = user,
        seeds = [b"state", user.key.as_ref()],
        bump,
        space = 8 + VaultState::INIT_SPACE
    )]
    // This account is a PDA, created using these seeds and bump.
    // It should be owned by the the current program and is used to store the bumps, with custom struct
    pub vault_state: Account<'info, VaultState>,

    #[account(
        seeds = [b"vault", user.key.as_ref()],
        bump
    )]
    // a systemAccount is a just for holding sol and no other data (can either be a keypair or a pda)
    // This account is a PDA, created using these seeds and bump.
    // It should be owned by the System Program, and just hold lamports (SOL), no custom struct
    // no need to "init" because a systemAccount is initialised automatically when we transfer enough sol to make the acc rent exempt
    pub vault: SystemAccount<'info>,

    // any account on chain is created by the system program
    // whenever you use the init constraint to create a new account, you must include the system program
    // in your accounts struct since it will be invoked to perform the account creation, later it gets assigned to custom program
    pub system_program: Program<'info, System>,
}

impl<'info> Initialize<'info> {
    // func to store initialise the PDAs and store the bumps on-chain in vault_statte pda
    pub fn initialize(&mut self, bumps: &InitializeBumps) -> Result<()> {
        self.vault_state.vault_bump = bumps.vault;
        self.vault_state.state_bump = bumps.vault_state;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct CloseAccounts<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"state", user.key.as_ref()],
        bump = vault_state.state_bump,
        // specifies where the rent will go after closing
        close = user
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        mut,
        seeds = [b"vault", user.key.as_ref()],
        bump = vault_state.vault_bump
    )]
    pub vault: SystemAccount<'info>,

    // system program is needed to transfer back the sol to the user and don't need to pay more rent
    // transfer all the
    pub system_program: Program<'info, System>,
}

impl<'info> CloseAccounts<'info> {
    pub fn close(&mut self) -> Result<()> {
        // this one is similar to withdraw function
        let cpi_program = self.system_program.to_account_info();
        let cpi_accounts = Transfer {
            from: self.vault.to_account_info(),
            to: self.user.to_account_info(),
        };

        let seeds: &[&[u8]] = &[
            b"vault",
            self.user.key.as_ref(),
            &[self.vault_state.vault_bump],
        ];

        let signer_seeds: &[&[&[u8]]] = &[&seeds[..]];

        let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

        // use lamports to get all the balance from the vault account
        transfer(cpi_context, self.vault.lamports())
    }
}

#[derive(Accounts)]
pub struct Payment<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [b"state", user.key.as_ref()],
        bump = vault_state.state_bump
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        mut,
        seeds = [b"vault", user.key.as_ref()],
        bump = vault_state.vault_bump
    )]
    // this is where the vault will actually be created because here we are depositing the sol
    pub vault: SystemAccount<'info>,

    // system program is needed to transfer the native sol
    pub system_program: Program<'info, System>,
}

impl<'info> Payment<'info> {
    pub fn deposit(&mut self, amount: u64) -> Result<()> {
        // transfer is not done by current program but the "system_program" using the CPI
        let cpi_program = self.system_program.to_account_info();
        let cpi_accounts = Transfer {
            from: self.user.to_account_info(),
            to: self.vault.to_account_info(),
        };

        // define the context to pass to the instruction of system_program using cpi
        // using "new" because the user will sign the tx
        // the user already signed for the ix in our program, so that sign is inherited to cpi
        let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
        transfer(cpi_context, amount)
    }

    pub fn withdraw(&mut self, amount: u64) -> Result<()> {
        let cpi_program = self.system_program.to_account_info();
        let cpi_accounts = Transfer {
            from: self.vault.to_account_info(),
            to: self.user.to_account_info(),
        };

        // When the CPI is processed, the Solana runtime will validate that the provided seeds and
        // caller program ID derive a valid PDA. The PDA is then added as a signer on the invocation.
        // "This mechanism allows for programs to sign for PDAs that are derived from their program ID."
        // “Hey, I control this PDA because I know the seeds used to generate it and it was created with my program ID.”
        let seeds: &[&[u8]] = &[
            b"vault",
            self.user.key.as_ref(),
            &[self.vault_state.vault_bump],
        ];

        let signer_seeds: &[&[&[u8]]] = &[&seeds[..]];

        // using "new_with_signer" because the pda will sign the tx now using the seeds
        let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
        transfer(cpi_context, amount)
    }
}

#[account]
pub struct VaultState {
    pub vault_bump: u8, // bump for the vault pda
    pub state_bump: u8, // bump for this pda itself
}

impl Space for VaultState {
    const INIT_SPACE: usize = 1 + 1;
}
