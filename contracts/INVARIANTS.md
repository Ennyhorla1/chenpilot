# Contract Invariants - Soroban Contract Suite

This document defines the invariants (safety properties) that must hold for each contract in the Chen Pilot Soroban suite.

## Table of Contents
- [Common Invariants](#common-invariants)
- [BTC Relay Invariants](#btc-relay-invariants)
- [RBAC Invariants](#rbac-invariants)
- [Multi-Hop Swap Invariants](#multi-hop-swap-invariants)
- [Flash Loan Guard Invariants](#flash-loan-guard-invariants)
- [Core Vault Invariants](#core-vault-invariants)
- [Fee Distribution Invariants](#fee-distribution-invariants)
- [HTLC Invariants](#htlc-invariants)
- [Intent Market Validator Invariants](#intent-market-validator-invariants)
- [Lending Liquidation Invariants](#lending-liquidation-invariants)
- [Liquidity Vault Invariants](#liquidity-vault-invariants)
- [PoR Validator Invariants](#por-validator-invariants)
- [Relayer Slashing Invariants](#relayer-slashing-invariants)
- [Strategy Registry Invariants](#strategy-registry-invariants)

---

## Common Invariants

### I1: Initialization State
**Invariant**: Contract can only be initialized once.

**Formal**: `initialized == true ⇒ cannot initialize again`

**Enforcement**: Instance storage flag checked on initialization.

### I2: Auth Requirement
**Invariant**: All state-changing functions require caller authentication.

**Formal**: `∀ f ∈ state_changing_functions: require_auth() is called`

**Enforcement**: Soroban SDK's `require_auth()` on privileged operations.

### I3: Storage Consistency
**Invariant**: Storage keys are unique and data is properly typed.

**Formal**: `∀ k ∈ storage_keys: type(value_at(k)) == expected_type(k)`

**Enforcement**: Rust type system and Soroban storage typing.

### I4: Event Emission
**Invariant**: All state changes emit corresponding events.

**Formal**: `∀ state_change: ∃ event_emission`

**Enforcement**: Event publishing after each state modification.

---

## BTC Relay Invariants

### BR1: Configuration Immutability
**Invariant**: Once initialized, config can only be changed by admin.

**Formal**: `config.admin == caller ⇒ can_update_config(config)`

**Enforcement**: `current.admin.require_auth()` on `update_config`.

### BR2: Claimed Transaction Uniqueness
**Invariant**: A Bitcoin transaction ID can only be claimed once.

**Formal**: `claimed(tx_id) == true ⇒ cannot_claim_again(tx_id)`

**Enforcement**: Persistent storage check before processing claim.

### BR3: Minimum Confirmations
**Invariant**: Merkle proof depth must meet minimum confirmations.

**Formal**: `merkle_proof.len() >= config.min_confirmations`

**Enforcement**: Depth check in `verify_and_claim`.

### BR4: Block Header Validity
**Invariant**: Block header must be exactly 80 bytes.

**Formal**: `block_header.len() == 80`

**Enforcement**: Length validation in `verify_and_claim`.

### BR5: Proof of Work
**Invariant**: Block header hash must meet target difficulty.

**Formal**: `hash(header) <= target(header)`

**Enforcement**: Delegated to btc_relay_crypto contract.

### BR6: Merkle Inclusion
**Invariant**: Transaction must be included in block via Merkle proof.

**Formal**: `merkle_root(header) == compute_merkle_root(tx_id, proof, index)`

**Enforcement**: Delegated to btc_relay_crypto contract.

### BR7: Claimed TTL
**Invariant**: Claimed records expire after TTL (~30 days).

**Formal**: `claimed(tx_id).expires_at <= current_ledger + CLAIMED_TX_TTL_LEDGERS`

**Enforcement**: `set_with_ttl` on claimed records.

---

## RBAC Invariants

### RB1: Super Admin Uniqueness
**Invariant**: Only one super admin exists at any time.

**Formal**: `|{a | is_super_admin(a)}| == 1`

**Enforcement**: Single instance storage slot for super admin.

### RB2: Role Grant Authorization
**Invariant**: Only super admin can grant roles.

**Formal**: `grant_role(to, role) ⇒ caller == super_admin`

**Enforcement**: `super_admin.require_auth()` on `grant_role`.

### RB3: Role Revoke Authorization
**Invariant**: Only super admin can revoke roles.

**Formal**: `revoke_role(from, role) ⇒ caller == super_admin`

**Enforcement**: `super_admin.require_auth()` on `revoke_role`.

### RB4: Role Consistency
**Invariant**: Role assignment is boolean (either has or doesn't have).

**Formal**: `has_role(addr, role) ∈ {true, false}`

**Enforcement**: Boolean storage type for role assignments.

### RB5: Admin Transfer Authorization
**Invariant**: Only current super admin can transfer admin.

**Formal**: `transfer_admin(new_admin) ⇒ caller == current_super_admin`

**Enforcement**: `old_admin.require_auth()` on `transfer_admin`.

### RB6: Role Check Correctness
**Invariant**: Role check returns accurate storage value.

**Formal**: `has_role(addr, role) == storage.get(HasRole(addr, role))`

**Enforcement**: Direct storage lookup in `has_role`.

---

## Multi-Hop Swap Invariants

### MHS1: Non-Empty Hops
**Invariant**: Swap must have at least one hop.

**Formal**: `hops.len() >= 1`

**Enforcement**: Empty check at start of `swap`.

### MHS2: Caller Authentication
**Invariant**: Only authenticated caller can initiate swap.

**Formal**: `swap(caller, hops) ⇒ caller.require_auth()`

**Enforcement**: `caller.require_auth()` at function start.

### MHS3: Slippage Protection
**Invariant**: Each hop output must meet minimum amount.

**Formal**: `∀ hop ∈ hops: amount_out >= hop.min_amount_out`

**Enforcement**: Slippage check after each swap.

### MHS4: Token Flow Conservation
**Invariant**: Tokens flow from caller → contract → pool → contract → caller.

**Formal**: `token_in(caller) → token_in(contract) → token_in(pool) → token_out(contract) → token_out(caller)`

**Enforcement**: Sequential transfer operations in swap loop.

### MHS5: Hop Sequence Integrity
**Invariant**: Hop output becomes next hop input.

**Formal**: `hop[i].amount_out == hop[i+1].amount_in`

**Enforcement**: Variable `current_amount` carries output to next hop.

### MHS6: Final Transfer
**Invariant**: Final output transferred to caller.

**Formal**: `last_hop.token_out.transfer(contract, caller, final_amount)`

**Enforcement**: Final transfer after loop completion.

---

## Flash Loan Guard Invariants

### FLG1: Configuration Immutability
**Invariant**: Config can only be changed by admin.

**Formal**: `update_config(config) ⇒ caller == current_admin`

**Enforcement**: `current.admin.require_auth()` on `update_config`.

### FLG2: Oracle Freshness
**Invariant**: Oracle data must not exceed staleness threshold.

**Formal**: `current_time <= oracle_timestamp + max_oracle_staleness_seconds`

**Enforcement**: Freshness check in `record_snapshot` and `assert_price_safe`.

### FLG3: Oracle Sequence Monotonicity
**Invariant**: Oracle sequence must strictly increase.

**Formal**: `new_oracle_sequence > last_oracle_sequence`

**Enforcement**: Sequence comparison in `record_snapshot`.

### FLG4: Minimum Ledger Gap
**Invariant**: Snapshots must respect minimum ledger gap.

**Formal**: `current_ledger >= last_snapshot_ledger + min_ledger_gap`

**Enforcement**: Ledger gap check in `record_snapshot`.

### FLG5: Price Deviation Limit
**Invariant**: Price deviation must stay within threshold.

**Formal**: `|current_price - snapshot_price| / snapshot_price <= max_intra_ledger_deviation_bps / 10000`

**Enforcement**: Deviation calculation and check in `assert_price_safe`.

### FLG6: Consecutive Price Change Limit
**Invariant**: Consecutive price changes must stay within threshold.

**Formal**: `|new_price - old_price| / old_price <= max_consecutive_price_change_bps / 10000`

**Enforcement**: Consecutive change check in `record_snapshot`.

### FLG7: Circuit Breaker Trigger
**Invariant**: Circuit breaker triggers after consecutive violations.

**Formal**: `consecutive_violations >= 3 ⇒ circuit_breaker.triggered == true`

**Enforcement**: Violation counting and trigger logic in `record_snapshot`.

### FLG8: Circuit Breaker Expiration
**Invariant**: Circuit breaker auto-releases after time window.

**Formal**: `current_time > trigger_timestamp + circuit_breaker_window_seconds ⇒ circuit_breaker.triggered == false`

**Enforcement**: Time-based release in `update_circuit_breaker`.

### FLG9: Snapshot TTL
**Invariant**: Price snapshot expires after TTL (~1 day).

**Formal**: `snapshot.expires_at <= current_ledger + PRICE_SNAPSHOT_TTL_LEDGERS`

**Enforcement**: `set_with_ttl` on snapshot storage.

### FLG10: Same-Ledger Rejection
**Invariant**: Cannot assert price safety in same ledger as snapshot.

**Formal**: `assert_price_safe() ⇒ current_ledger != snapshot.ledger`

**Enforcement**: Ledger comparison in `assert_price_safe`.

---

## Core Vault Invariants

### CV1: Withdrawal Authorization
**Invariant**: Withdrawals require proper authorization.

**Formal**: `withdraw(amount, recipient) ⇒ caller.has_role(Withdrawer) || caller == recipient`

**Enforcement**: RBAC check or ownership check.

### CV2: Collateral Sufficiency
**Invariant**: Borrow amount must not exceed collateral limit.

**Formal**: `borrow_amount <= collateral_value * collateral_factor`

**Enforcement**: Collateral ratio check before borrow.

### CV3: Solvency
**Invariant**: Total assets >= total liabilities.

**Formal**: `total_assets >= total_borrows + total_interest`

**Enforcement**: Balance checks on operations.

### CV4: Interest Accrual
**Invariant**: Interest accrues on outstanding borrows.

**Formal**: `borrow_balance(t) = borrow_balance(t-1) * (1 + interest_rate)`

**Enforcement**: Interest calculation on each ledger.

---

## Fee Distribution Invariants

### FD1: Distribution Authorization
**Invariant**: Fee distribution requires admin authorization.

**Formal**: `distribute_fees() ⇒ caller == admin`

**Enforcement**: Admin auth check on distribution.

### FD2: Fee Pool Non-Negativity
**Invariant**: Fee pool balance cannot go negative.

**Formal**: `fee_pool_balance >= 0`

**Enforcement**: Balance check before distribution.

### FD3: Recipient Validation
**Invariant**: Fee recipients must be whitelisted or admin-approved.

**Formal**: `is_valid_recipient(recipient) == true ⇒ can_distribute_to(recipient)`

**Enforcement**: Recipient whitelist check.

---

## HTLC Invariants

### HTLC1: Hash Lock Security
**Invariant**: Funds can only be claimed with valid preimage.

**Formal**: `claim(preimage) ⇒ hash(preimage) == lock_hash`

**Enforcement**: Hash verification on claim.

### HTLC2: Time Lock Expiration
**Invariant**: Funds can be refunded after time lock expires.

**Formal**: `current_time >= time_lock_expiry ⇒ can_refund()`

**Enforcement**: Time comparison on refund.

### HTLC3: Claim Uniqueness
**Invariant**: Each contract instance can only be claimed once.

**Formal**: `claimed == true ⇒ cannot_claim_again()`

**Enforcement**: Claimed flag in storage.

### HTLC4: Refund Uniqueness
**Invariant**: Each contract instance can only be refunded once.

**Formal**: `refunded == true ⇒ cannot_refund_again()`

**Enforcement**: Refunded flag in storage.

---

## Intent Market Validator Invariants

### IMV1: Intent Signature Validity
**Invariant**: Intent must have valid signature.

**Formal**: `verify_signature(intent, signature) == true ⇒ can_validate(intent)`

**Enforcement**: Signature verification before validation.

### IMV2: Intent Nonce Uniqueness
**Invariant**: Each intent nonce can only be used once.

**Formal**: `used_nonce(nonce) == true ⇒ cannot_use_again(nonce)`

**Enforcement**: Nonce tracking in storage.

### IMV3: Intent Expiration
**Invariant**: Intents expire after timeout.

**Formal**: `current_time > intent.expiry ⇒ cannot_validate(intent)`

**Enforcement**: Timestamp check on validation.

---

## Lending Liquidation Invariants

### LL1: Liquidation Threshold
**Invariant**: Position can only be liquidated if collateral ratio below threshold.

**Formal**: `collateral_ratio < liquidation_threshold ⇒ can_liquidate(position)`

**Enforcement**: Ratio check before liquidation.

### LL2: Liquidation Coverage
**Invariant**: Liquidation must cover debt plus penalty.

**Formal**: `liquidation_amount >= debt + liquidation_penalty`

**Enforcement**: Amount calculation on liquidation.

### LL3: Liquidator Reward
**Invariant**: Liquidator receives reward for successful liquidation.

**Formal**: `liquidator_reward = liquidation_amount * liquidation_incentive`

**Enforcement**: Reward calculation on liquidation.

---

## Liquidity Vault Invariants

### LV1: Constant Product Formula
**Invariant**: Token reserves maintain constant product (x * y = k).

**Formal**: `reserve_a * reserve_b == k (constant)`

**Enforcement**: Formula validation on swap.

### LV2: LP Token Supply
**Invariant**: LP token supply matches total liquidity.

**Formal**: `total_lp_tokens == total_liquidity / initial_share_price`

**Enforcement**: Supply calculation on mint/burn.

### LV3: Reserve Balance
**Invariant**: Contract reserves match actual token balances.

**Formal**: `reserve_a == token_a.balance(contract)`

**Enforcement**: Balance verification on operations.

### LV4: Slippage Protection
**Invariant**: Swap output must meet minimum amount.

**Formal**: `amount_out >= min_amount_out`

**Enforcement**: Slippage check on swap.

---

## PoR Validator Invariants

### PoR1: Proof Signature Validity
**Invariant**: Proof of reserve must have valid signature.

**Formal**: `verify_signature(por, signature) == true ⇒ can_validate(por)`

**Enforcement**: Signature verification on validation.

### PoR2: Reserve Balance Accuracy
**Invariant**: Reported reserve balance must match actual on-chain balance.

**Formal**: `reported_balance == on_chain_balance`

**Enforcement**: Balance verification on validation.

### PoR3: Proof Freshness
**Invariant**: Proof of reserve must be recent.

**Formal**: `current_time - proof_timestamp <= max_staleness`

**Enforcement**: Timestamp check on validation.

---

## Relayer Slashing Invariants

### RS1: Slashing Condition
**Invariant**: Relayer can only be slashed for valid violations.

**Formal**: `slash(relayer) ⇒ violation_detected(relayer) == true`

**Enforcement**: Violation check before slashing.

### RS2: Slashing Amount
**Invariant**: Slashing amount must not exceed bonded stake.

**Formal**: `slash_amount <= relayer.bonded_stake`

**Enforcement**: Amount check on slashing.

### RS3: Slashing Uniqueness
**Invariant**: Each violation can only result in one slash.

**Formal**: `slashed_for(violation_id) == true ⇒ cannot_slash_again(violation_id)`

**Enforcement**: Violation tracking in storage.

---

## Strategy Registry Invariants

### SR1: Strategy Approval
**Invariant**: Strategy must be approved before use.

**Formal**: `is_approved(strategy) == true ⇒ can_use(strategy)`

**Enforcement**: Approval check before strategy execution.

### SR2: Strategy Uniqueness
**Invariant**: Each strategy ID is unique.

**Formal**: `strategy_id1 == strategy_id2 ⇒ strategy1 == strategy2`

**Enforcement**: Unique ID assignment on registration.

### SR3: Strategy Performance
**Invariant**: Strategy performance metrics are accurately tracked.

**Formal**: `strategy.performance == actual_returns / expected_returns`

**Enforcement**: Performance calculation on each execution.

---

## Invariant Testing

### Property-Based Testing
Several contracts include property-based tests to verify invariants:

- **Flash Loan Guard**: `test_invariants.rs` - Tests price deviation, circuit breaker, and freshness invariants
- **Liquidity Vault**: `test_property.rs` - Tests constant product formula and reserve balance invariants

### Unit Test Coverage
All contracts include unit tests in `test.rs` that verify:
- Initialization invariants
- Access control invariants
- State transition invariants
- Boundary condition invariants

### Fuzzing Recommendations
For comprehensive invariant verification, consider:
- Fuzzing public entry points with random inputs
- Fuzzing cross-contract call sequences
- Fuzzing storage state transitions
- Fuzzing timestamp/ledger sequence manipulation

---

## Conclusion

These invariants define the safety properties that must hold for the Soroban contract suite. Auditors should verify that:

1. Each invariant is properly enforced in the code
2. Invariant violations are impossible or result in panic
3. Invariant violations emit appropriate events
4. Invariant testing covers edge cases
5. Cross-contract interactions preserve invariants

Any violation of these invariants represents a potential security vulnerability and should be addressed before deployment.
