// ────────────────────────────────────────────────────────────────────────────────

#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Env, Address, BytesN, token, symbol_short, Symbol};
use contract_failure::{fail, unwrap_or_fail, FailureReason};

// Unified role enum - must match UnifiedAuth contract
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum UnifiedRole {
    SuperAdmin,
    EmergencyAdmin,
    VaultAdmin,
    UpgradeAdmin,
    StrategyAdmin,
    RoleAdmin,
    OracleProvider,
    AgentOperator,
    StrategyOperator,
}

// ~1 hour at 5s/ledger
const TIMELOCK_LEDGERS: u32 = 720;

// 48 hours at 5s/ledger
const FORCE_EXIT_DELAY: u64 = 172_800;

// TTL for force-exit requests: ~50 hours (360000 ledgers at 5s/ledger)
const FORCE_EXIT_TTL_LEDGERS: u32 = 360_000;

// TTL for deposit records: ~7 days (persistent, but with TTL extension for active accounts)
const DEPOSIT_TTL_LEDGERS: u32 = 1_209_600;

// ─── Events ─────────────────────────────────────────────────────────────────────

const EVT_INIT: Symbol = symbol_short!("init");
const EVT_DEPOSIT: Symbol = symbol_short!("deposit");
const EVT_WITHDRAWAL: Symbol = symbol_short!("w");
const EVT_FORCE_EXIT_REQ: Symbol = symbol_short!("fexit_req");
const EVT_FORCE_EXIT_CMPL: Symbol = symbol_short!("fexit_c");
const EVT_RECOVERY: Symbol = symbol_short!("recovery");
const EVT_UPG_PROP: Symbol = symbol_short!("upg_prop");
const EVT_UPG_CNCL: Symbol = symbol_short!("upg_cncl");
const EVT_UPG_DONE: Symbol = symbol_short!("upg_done");
const EVT_ADM_XFER: Symbol = symbol_short!("adm_xfer");

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    // Legacy admin for backward compatibility
    Admin,
    // UnifiedAuth integration
    UnifiedAuth,
    PendingUpgrade,
    BackendOnline,
    Deposit(Address),
    ForceExit(Address),
    VaultToken,
    // Upgrade mode flag
    UpgradeMode,
}

#[contracttype]
#[derive(Clone)]
pub struct PendingUpgrade {
    pub new_wasm_hash: BytesN<32>,
    pub unlock_ledger: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct ForceExitRequest {
    pub amount: i128,
    pub eligible_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct DepositEvent {
    pub user: Address,
    pub amount: i128,
    pub total_deposited: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct WithdrawalEvent {
    pub user: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct RecoveryEvent {
    pub user: Address,
    pub amount: i128,
    pub reason: RecoveryReason,
}

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum RecoveryReason {
    ForceExitTimeout,
    AdminIntervention,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtInit {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub admin: Address,
    pub vault_token: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtBackendStatus {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub online: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtDeposit {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub user: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtForceExitReq {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub user: Address,
    pub amount: i128,
    pub eligible_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtForceExitDone {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub user: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtUpgProp {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub new_wasm_hash: BytesN<32>,
    pub unlock_ledger: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtUpgCncl {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtUpgDone {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub new_wasm_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtAdmXfer {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub old_admin: Address,
    pub new_admin: Address,
}

#[contract]
pub struct CoreVaultContract;

#[contractimpl]
impl CoreVaultContract {
    /// Initialize the contract with an admin, vault token, and unified auth address.
    /// Emits `init` event on success.
    pub fn init(env: Env, admin: Address, vault_token: Address, unified_auth: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            fail(&env, FailureReason::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::VaultToken, &vault_token);
        env.storage().instance().set(&DataKey::UnifiedAuth, &unified_auth);
        env.storage().instance().set(&DataKey::BackendOnline, &true);
        env.storage().instance().set(&DataKey::UpgradeMode, &false);
        env.events().publish(
            (EVT_INIT,),
            admin.clone(),
        );
    }

    // ── Backend status ────────────────────────────────────────────────────────

    /// Admin marks the backend as offline, enabling force-exit requests.
    /// Trust boundary: VAULT_ADMIN or EMERGENCY_ADMIN - users cannot manipulate backend status.
    pub fn set_backend_status(env: Env, online: bool) {
        Self::require_vault_or_emergency_admin(&env);
        
        // Check emergency state - cannot change backend status during emergency
        if Self::is_emergency_active(&env) {
            fail(&env, FailureReason::EmergencyMode);
        }
        
        // Check upgrade mode - restrict backend changes during upgrade
        if Self::is_upgrade_mode(&env) {
            fail(&env, FailureReason::UpgradeMode);
        }
        
        let caller = env.current_contract_address();
        env.storage().instance().set(&DataKey::BackendOnline, &online);

        env.events().publish(
            (symbol_short!("vault"), symbol_short!("backend_status")),
            EvtBackendStatus {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: caller,
                online,
            },
        );
    }

    pub fn is_backend_online(env: Env) -> bool {
        env.storage().instance().get(&DataKey::BackendOnline).unwrap_or(true)
    }

    // ── Deposit ───────────────────────────────────────────────────────────────

    /// Deposit tokens into the vault.
    /// Only allowed when backend is ONLINE.
    /// Trust boundary: Funds are non-custodial - user owns their deposit balance.
    /// Emits `deposit` event on success.
    pub fn deposit(env: Env, user: Address, amount: i128) {
        user.require_auth();
        if !Self::is_backend_online(env.clone()) {
            fail(&env, FailureReason::BackendOffline);
        }

        let vault_token: Address =
            unwrap_or_fail(&env, env.storage().instance().get(&DataKey::VaultToken), FailureReason::StorageValueMissing);
        let token_client = token::Client::new(&env, &vault_token);
        token_client.transfer(&user, &env.current_contract_address(), &amount);

        let current: i128 = env.storage().persistent()
            .get(&DataKey::Deposit(user.clone()))
            .unwrap_or(0);
        let new_balance = current + amount;
        
        // Store deposit with TTL extension to keep active accounts fresh
        env.storage().persistent().set(&DataKey::Deposit(user.clone()), &new_balance);
        env.storage().persistent().extend_ttl(&DataKey::Deposit(user.clone()), 100, DEPOSIT_TTL_LEDGERS);

        env.events().publish(
            (EVT_DEPOSIT, user.clone()),
            DepositEvent { user, amount, total_deposited: new_balance },
        );
    }

    // ── Withdrawal ────────────────────────────────────────────────────────────

    /// Normal withdrawal when backend is ONLINE.
    /// Trust boundary: Users can withdraw their own balance; contract holds no custody.
    /// Emits `w` event on success.
    pub fn withdrawal(env: Env, user: Address, amount: i128) {
        user.require_auth();
        if !Self::is_backend_online(env.clone()) {
            fail(&env, FailureReason::BackendOffline);
        }

        let balance: i128 = env.storage().persistent()
            .get(&DataKey::Deposit(user.clone()))
            .unwrap_or(0);
        if balance < amount {
            fail(&env, FailureReason::InsufficientBalance);
        }

        let new_balance = balance - amount;
        if new_balance == 0 {
            env.storage().persistent().remove(&DataKey::Deposit(user.clone()));
        } else {
            env.storage().persistent().set(&DataKey::Deposit(user.clone()), &new_balance);
            env.storage().persistent().extend_ttl(&DataKey::Deposit(user.clone()), 100, DEPOSIT_TTL_LEDGERS);
        }

        let vault_token: Address =
            unwrap_or_fail(&env, env.storage().instance().get(&DataKey::VaultToken), FailureReason::StorageValueMissing);
        let token_client = token::Client::new(&env, &vault_token);
        token_client.transfer(&env.current_contract_address(), &user, &amount);

        env.events().publish(
            (EVT_WITHDRAWAL, user.clone()),
            WithdrawalEvent { user, amount },
        );
    }

    // ── Force Exit ────────────────────────────────────────────────────────────

    /// Initiate a force-exit. Only allowed when backend is OFFLINE.
    /// Starts the 48-hour challenge period.
    /// Trust boundary: Escape hatch for users when backend is unavailable.
    /// Emits `fexit_req` event on success.
    pub fn force_exit_request(env: Env, user: Address) {
        user.require_auth();
        if Self::is_backend_online(env.clone()) {
            fail(&env, FailureReason::BackendOnline);
        }

        let balance: i128 = env.storage().persistent()
            .get(&DataKey::Deposit(user.clone()))
            .unwrap_or(0);
        if balance <= 0 {
            fail(&env, FailureReason::InsufficientBalance);
        }

        // Prevent duplicate requests
        if env.storage().persistent().has(&DataKey::ForceExit(user.clone())) {
            fail(&env, FailureReason::ForceExitAlreadyPending);
        }

        let eligible_at = env.ledger().timestamp() + FORCE_EXIT_DELAY;
        let req = ForceExitRequest { amount: balance, eligible_at };
        
        // Store force-exit with TTL to auto-expire unclaimed requests after ~50 hours
        // This prevents stale requests from accumulating indefinitely
        env.storage().persistent().set(&DataKey::ForceExit(user.clone()), &req);
        env.storage().persistent().extend_ttl(&DataKey::ForceExit(user.clone()), 100, FORCE_EXIT_TTL_LEDGERS);

        env.events().publish(
            (EVT_FORCE_EXIT_REQ, user),
            req,
        );
    }

    /// Complete the force-exit after the 48-hour challenge period has passed.
    /// Trust boundary: Users can claim their own funds after challenge period.
    /// Emits `fexit_c` event on success.
    pub fn force_exit_complete(env: Env, user: Address) {
        user.require_auth();

        let req: ForceExitRequest = env.storage().persistent()
            .get(&DataKey::ForceExit(user.clone()))
            .unwrap_or_else(|| fail(&env, FailureReason::NoPendingForceExit));

        if env.ledger().timestamp() < req.eligible_at {
            fail(&env, FailureReason::ChallengePeriodNotElapsed);
        }

        // Clear state before transfer (re-entrancy guard)
        env.storage().persistent().remove(&DataKey::ForceExit(user.clone()));
        env.storage().persistent().remove(&DataKey::Deposit(user.clone()));

        let vault_token: Address =
            unwrap_or_fail(&env, env.storage().instance().get(&DataKey::VaultToken), FailureReason::StorageValueMissing);
        let token_client = token::Client::new(&env, &vault_token);
        token_client.transfer(&env.current_contract_address(), &user, &req.amount);

        env.events().publish(
            (EVT_FORCE_EXIT_CMPL, user),
            req,
        );
    }

    /// Recovery: Cancel a pending force-exit request.
    /// Only callable by VAULT_ADMIN or EMERGENCY_ADMIN.
    /// Trust boundary: ADMIN emergency function for edge cases.
    /// Emits `recovery` event on success.
    /// Note: During force_exit_request, the deposit is NOT removed. It is only removed
    /// during force_exit_complete. This function cancels the pending request without
    /// modifying the deposit balance.
    pub fn recovery(env: Env, user: Address) {
        Self::require_vault_or_emergency_admin(&env);

        let req: ForceExitRequest = env.storage().persistent()
            .get(&DataKey::ForceExit(user.clone()))
            .unwrap_or_else(|| fail(&env, FailureReason::NoPendingForceExit));

        // Validate state consistency
        let deposit: i128 = env.storage().persistent()
            .get(&DataKey::Deposit(user.clone()))
            .unwrap_or(0);
        if deposit != req.amount {
            fail(&env, FailureReason::DepositForceExitMismatch);
        }

        // Remove force exit request - deposit balance remains unchanged
        env.storage().persistent().remove(&DataKey::ForceExit(user.clone()));

        env.events().publish(
            (EVT_RECOVERY, user.clone()),
            RecoveryEvent { user, amount: req.amount, reason: RecoveryReason::AdminIntervention },
        );
    }

    /// Returns a pending force-exit request for a user, if any
    pub fn get_force_exit(env: Env, user: Address) -> Option<ForceExitRequest> {
        env.storage().persistent().get(&DataKey::ForceExit(user))
    }

    /// Returns the current deposit balance for a user
    pub fn get_deposit(env: Env, user: Address) -> Option<i128> {
        env.storage().persistent().get(&DataKey::Deposit(user))
    }

    // ── Upgrade time-lock ─────────────────────────────────────────────────────

    /// Propose a contract upgrade. Starts ~1 hour timelock.
    /// Trust boundary: UPGRADE_ADMIN ONLY - upgrade is time-locked, not immediately executable.
    /// Enters upgrade mode which restricts critical state changes.
    /// Emits `upg_prop` event on success.
    pub fn propose_upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        Self::require_upgrade_admin(&env);
        
        // Check emergency state - cannot upgrade during emergency
        if Self::is_emergency_active(&env) {
            fail(&env, FailureReason::EmergencyMode);
        }

        let unlock_ledger = env.ledger().sequence() + TIMELOCK_LEDGERS;
        let pending = PendingUpgrade { new_wasm_hash: new_wasm_hash.clone(), unlock_ledger };
        env.storage().instance().set(&DataKey::PendingUpgrade, &pending);
        
        // Enter upgrade mode - restricts critical state changes
        env.storage().instance().set(&DataKey::UpgradeMode, &true);

        let caller = env.current_contract_address();
        env.events().publish(
            (EVT_UPG_PROP, caller),
            pending,
        );
    }

    /// Cancel a pending upgrade.
    /// Trust boundary: UPGRADE_ADMIN ONLY - before timelock expires.
    /// Exits upgrade mode and restores normal operations.
    /// Emits `upg_cncl` event on success.
    pub fn cancel_upgrade(env: Env) {
        Self::require_upgrade_admin(&env);
        env.storage().instance().remove(&DataKey::PendingUpgrade);
        
        // Exit upgrade mode
        env.storage().instance().set(&DataKey::UpgradeMode, &false);

        let caller = env.current_contract_address();
        env.events().publish(
            (EVT_UPG_CNCL, caller),
            caller,
        );
    }

    /// Apply the pending upgrade after timelock has elapsed.
    /// Trust boundary: Anyone can call after timelock expires (admin or public).
    /// Exits upgrade mode after successful upgrade.
    /// Emits `upg_done` event on success.
    pub fn apply_upgrade(env: Env) {
        let pending: PendingUpgrade = env
            .storage()
            .instance()
            .get(&DataKey::PendingUpgrade)
            .unwrap_or_else(|| fail(&env, FailureReason::NoPendingUpgrade));

        if env.ledger().sequence() < pending.unlock_ledger {
            fail(&env, FailureReason::TimelockNotExpired);
        }

        let wasm_hash = pending.new_wasm_hash.clone();
        env.storage().instance().remove(&DataKey::PendingUpgrade);
        
        // Exit upgrade mode after successful upgrade
        env.storage().instance().set(&DataKey::UpgradeMode, &false);
        
        env.deployer().update_current_contract_wasm(wasm_hash.clone());

        env.events().publish(
            (EVT_UPG_DONE,),
            wasm_hash,
        );
    }

    /// Transfer admin role to a new address.
    /// Trust boundary: CURRENT ADMIN ONLY (legacy) - use UnifiedAuth for role management.
    /// Emits `adm_xfer` event on success.
    pub fn transfer_admin(env: Env, new_admin: Address) {
        let admin: Address =
            unwrap_or_fail(&env, env.storage().instance().get(&DataKey::Admin), FailureReason::StorageValueMissing);
        admin.require_auth();

        let old_admin = admin.clone();
        env.storage().instance().set(&DataKey::Admin, &new_admin);

        env.events().publish(
            (EVT_ADM_XFER,),
            (old_admin, new_admin),
        );
    }
    
    /// Set the UnifiedAuth contract address.
    /// Trust boundary: CURRENT ADMIN ONLY.
    pub fn set_unified_auth(env: Env, unified_auth: Address) {
        let admin: Address =
            unwrap_or_fail(&env, env.storage().instance().get(&DataKey::Admin), FailureReason::StorageValueMissing);
        admin.require_auth();
        env.storage().instance().set(&DataKey::UnifiedAuth, &unified_auth);
    }
    
    // ─── UnifiedAuth Integration Helpers ─────────────────────────────────────
    
    fn get_unified_auth(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::UnifiedAuth)
            .unwrap_or_else(|| fail(env, FailureReason::UnifiedAuthNotConfigured))
    }
    
    fn require_vault_or_emergency_admin(env: &Env) {
        let unified_auth = Self::get_unified_auth(env);
        let caller = env.current_contract_address();
        
        // Try VaultAdmin first, then EmergencyAdmin
        // In production, this would call UnifiedAuth contract
        // For now, fall back to legacy admin check
        let admin: Address =
            unwrap_or_fail(env, env.storage().instance().get(&DataKey::Admin), FailureReason::StorageValueMissing);
        admin.require_auth();
    }
    
    fn require_upgrade_admin(env: &Env) {
        let unified_auth = Self::get_unified_auth(env);
        let caller = env.current_contract_address();
        
        // In production, call UnifiedAuth::require_role(UnifiedRole::UpgradeAdmin)
        // For now, fall back to legacy admin check
        let admin: Address =
            unwrap_or_fail(env, env.storage().instance().get(&DataKey::Admin), FailureReason::StorageValueMissing);
        admin.require_auth();
    }
    
    fn is_emergency_active(env: &Env) -> bool {
        let unified_auth = Self::get_unified_auth(env);
        // In production, call UnifiedAuth::is_emergency_active()
        // For now, return false
        false
    }
    
    fn is_upgrade_mode(env: &Env) -> bool {
        env.storage().instance().get(&DataKey::UpgradeMode).unwrap_or(false)
    }

    pub fn upgrade_unlock_ledger(env: Env) -> u32 {
        env.storage()
            .instance()
            .get::<DataKey, PendingUpgrade>(&DataKey::PendingUpgrade)
            .map(|p| p.unlock_ledger)
            .unwrap_or(0)
    }
    
    pub fn is_upgrade_mode(env: Env) -> bool {
        Self::is_upgrade_mode(&env)
    }
    
    pub fn get_unified_auth(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::UnifiedAuth)
    }
}

mod test;
