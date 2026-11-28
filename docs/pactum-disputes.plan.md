# `pactum-disputes` Implementation Plan

> **Status:** Approved design — ready for implementation  
> **Program:** `programs/pactum-disputes` (replacing legacy `programs/stayke-disputes`)  
> **References:** [`pactum-programs-spec.md`](../../docs/architecture/pactum-programs-spec.md), [`pactum-jury-sortition-tree.md`](../../docs/architecture/pactum-jury-sortition-tree.md), [`pactum-design-notes.md`](../../docs/architecture/pactum-design-notes.md)

---

## 1. Scope & Ownership Split

### Agent Scope
- Workspace migration: rename `stayke-disputes` to `pactum-disputes`, register in root `Cargo.toml` and `Anchor.toml`.
- Config & state: `DisputeConfig`, `Dispute`, `VoteRecord`, `cpi_authority` PDA (`AllowedCaller::Disputes`), errors, and events.
- **Level 1 — P2P:** `open_dispute`, `link_evidence`, `solve_p2p_agreement`, `escalate_dispute`.
- **Level 2A — `ArbiterRegistry`:** `resolve_by_arbiter`, `propose_arbiter_replacement`, `accept_arbiter_replacement`, `fallback_to_integrator`.
- **Level 2B — Jury Tally:** `tally_or_trigger_runoff`.
- **Cleanup:** `close_dispute`.
- **Tests:** Unit tests and atomic LiteSVM integration tests (1 file per instruction).

### User Scope (Minimal Account Definitions / Stubs Only for Compilation)
- **`JuryPool` & `SortitionSumTree`:** `JuryPool` (`#[account(zero_copy)]`), `PoolLeaf`, `SortitionSumTree` operations (`insert`/`update`/`remove`/`draw` with `vacant_stack`), and `join_jury_pool`, `sync_judge_pool`, `leave_jury_pool`.
- **MagicBlock VRF:** `request_jury_vrf` and `consume_jury_vrf` (`#[vrf]`) populating `Dispute.selected_leaves` and updating `Judge.active_disputes`.
- **Jury Voting (Commit-Reveal):** `commit_vote` and `reveal_vote` using deterministic wallet-signature salt derivation.

*(Note: `pactum-treasury` will NOT make CPIs into `pactum-disputes` to avoid circular Cargo/Anchor dependencies. Pool sync lives in `pactum-disputes` and composes atomically in the same Solana transaction alongside `stake_judge` / `unstake_judge`.)*

---

## 2. Agreed Economic & Settlement Rules

1. **Symmetric `Offer.bond` & Separate Refundable `jury_fee` Deposit (`pactum-core` & `pactum-escrow`):**
   - `Offer.bond: u64` is a single symmetric guarantee (can be `0`); both `client_bond` and `provider_bond` equal `offer.bond` and serve strictly as counterparty performance/cancellation protection.
   - Separately, each party deposits a refundable `jury_fee` reserve equal to `max(MIN_JURY_FEE (2_000_000), marketplace.min_jury_fee, offer.jury_fee)` (2 USDC floor) into `EscrowVault`.
   - Happy path or P2P settlement: 100% of both `bond` and `jury_fee` deposits are returned to `client` and `provider`.
2. **No `dispute_escalation_fee`:**
   - `escalate_dispute` does not charge an extra protocol escalation fee; the arbiter/jury fee itself acts as the anti-spam filter.
3. **Verdict Fee Allocation:**
   - **`ClientWins` / `ProviderWins`:** The losing party's refundable `jury_fee` deposit covers `arbiter_fee` or `jury_fee`. The winning party receives 100% of their disputed principal share, 100% of their own `bond`, and 100% of their own `jury_fee` deposit.
   - **`NoFault`:** Only the party who opened the dispute (`Dispute.opened_by`) pays a discounted fee from their `jury_fee` deposit: `fee * DisputeConfig.no_fault_fee_bps / 10_000` (default `5000` = 50%). The counterparty receives 100% of their `bond` and `jury_fee` deposit.
4. **`DisputeMode::Hybrid` Routing & Market Rules:**
   - `ArbiterOnly`: Requires `agreement.arbiter.is_some()` (Level 2A only).
   - `DecentralizedJury`: Requires `agreement.arbiter.is_none()` (Level 2B only).
   - `Hybrid`: Routes per agreement — `agreement.arbiter.is_some()` uses Level 2A (`ArbiterRegistry`), and `agreement.arbiter.is_none()` uses Level 2B (`DecentralizedJury`). If a Level 2A arbiter breaches SLA or is revoked and neither bilateral replacement nor `Integrator` fallback succeeds, `Hybrid` falls back to `DecentralizedJury` (VRF) instead of an extreme split.
5. **Arbiter SLA Breach / Revocation Slashing:**
   - Default slash: `DisputeConfig.arbiter_sla_slash_bps = 500` (5% of `arbiter.staked_amount`) per unattended dispute after `sla_deadline` or revocation.
   - Execution timing: `cpi_slash_stake` and `cpi_update_active_disputes(ParticipantRole::Arbiter, -1)` execute **only when the replacement or fallback is accepted** (`accept_arbiter_replacement` / `fallback_to_integrator`), keeping the defaulting arbiter's stake locked in `pactum-treasury` until transfer.
   - Destination: transferred directly to the replacement `Arbiter` or fallback `Integrator` to subsidize taking over the abandoned dispute (never to `platform_vault` or the disputing parties).

---

## 3. Accounts & PDA Seeds

| Account | Seeds | Key Fields |
|---|---|---|
| **`DisputeConfig`** | `["dispute_config"]` | `authority`, `global_config`, `p2p_window_secs`, `vote_window_secs`, `reveal_window_secs`, `arbiter_sla_slash_bps` (`500`), `no_fault_fee_bps` (`5000`), `is_initialized`, `bump` |
| **`Dispute`** | `["dispute", agreement]` | `agreement`, `marketplace`, `client`, `provider`, `opened_by: DisputeParty`, `mode: DisputeMode` (`Arbiter` \| `DecentralizedJury`), `assigned_arbiter: Option<Pubkey>`, `proposed_replacement_arbiter: Option<Pubkey>`, `selected_leaves: [u32; 5]`, `panel_size: u8`, `round: u8` (`1` \| `2`), `runoff_choices: Option<[VoteChoice; 2]>`, `client_evidence: Option<[u8; 32]>`, `provider_evidence: Option<[u8; 32]>`, `status: DisputeStatus`, `created_at: i64`, `phase_started_at: i64`, `sla_deadline: i64`, `rent_payer: Pubkey`, `bump: u8` |
| **`VoteRecord`** | `["vote_record", dispute, round, leaf_index]` | `commit_hash: [u8; 32]`, `revealed_choice: Option<VoteChoice>`, `has_revealed: bool`, `rent_payer: Pubkey`, `bump: u8` |
| **`JuryPool`** *(User stub)* | `["jury_pool", marketplace, pool_id]` | `#[account(zero_copy)]` `SortitionSumTree` (`nodes`, `vacant_stack`, `vacant_count`, `next_leaf`) |
| **`PoolLeaf`** *(User stub)* | `["pool_leaf", marketplace, pool_id, leaf_index]` | `judge: Pubkey`, `rent_payer: Pubkey`, `bump: u8` |

---

## 4. Instructions & Downstream CPI Wiring

### Admin
- `initialize_dispute_config(params)` / `update_dispute_config(params)` (`global_config.authority` signer).

### Level 1 — P2P (`$0` fee)
- `open_dispute`: Creates `Dispute`, sets `mode` from `Marketplace.dispute_mode` / `Agreement.arbiter`, and calls `pactum_escrow::cpi::cpi_freeze_agreement`.
- `link_evidence(evidence_hash: [u8; 32])`: Anchors `client_evidence` or `provider_evidence`.
- `solve_p2p_agreement(params)`: Bilateral settlement calling `pactum_escrow::cpi::cpi_settle_disputed_agreement` (either restoring previous status or settling split with `fee_payout = 0`).
- `escalate_dispute`: After `p2p_window_secs` (or bilateral escalation), transitions to Level 2A (`Arbiter`, calling `pactum_core::cpi::cpi_update_active_disputes(+1)` and setting `sla_deadline`) or Level 2B (`DecentralizedJury`).

### Level 2A — `ArbiterRegistry` & Fallback
- `resolve_by_arbiter(outcome)`: Signed by `assigned_arbiter` (`ArbiterProfile.is_active`), settles escrow + `arbiter_fee` via `cpi_settle_disputed_agreement`, updates `pactum-core` (`cpi_record_dispute_result`, `cpi_record_arbiter_resolution`, `cpi_update_active_disputes(-1)`).
- `propose_arbiter_replacement(new_arbiter)`: Available when `sla_deadline` has elapsed or `assigned_arbiter` is inactive (`!is_active`).
- `accept_arbiter_replacement`: Counterparty accepts replacement; executes `pactum_treasury::cpi::cpi_slash_stake` (5% to replacement arbiter) and `cpi_update_active_disputes(-1)` on old arbiter, `+1` on new arbiter, and resets `sla_deadline`.
- `fallback_to_integrator`: Escalates to `IntegratorProfile` (or extreme-exit fallback if none) when SLA is breached / arbiter revoked, slashing 5% to the integrator via `cpi_slash_stake` and settling via `cpi_settle_disputed_agreement`.

### Level 2B — Jury Commit-Reveal & Tally/Runoff
- **Deterministic Salt Derivation (Off-Chain Client Standard):** To avoid local storage loss across devices/browsers, the client derives `salt = sha256(signMessage("pactum-vote:<dispute>:<round>"))` and computes `commit_hash = sha256(dispute, round, leaf_index, choice, salt)`. On reveal, re-signing the same message deterministically recovers `salt` (and testing the 2–3 `VoteChoice` variants against `VoteRecord.commit_hash` recovers `choice`).
- `commit_vote(leaf_index, commit_hash)` *(User scope)*: Selected judge (verified via `PoolLeaf` and `selected_leaves`) stores `VoteRecord.commit_hash` during `vote_window_secs`.
- `reveal_vote(leaf_index, choice, salt)` *(User scope)*: Verifies `sha256(dispute, round, leaf_index, choice, salt) == commit_hash` (and `runoff_choices` in Round 2) during `reveal_window_secs`.
- `tally_or_trigger_runoff`: Permissionless after `reveal_window_secs`:
  - **Absolute majority:** Settles escrow + `jury_fee` via `cpi_settle_disputed_agreement`, records `cpi_record_judge_vote` and `cpi_record_dispute_result`, decrements `active_disputes(-1)`, and slashes absent/non-revealing/off-consensus judges via `cpi_slash_stake`.
  - **Round 1 tie:** Sets `round = 2`, stores top-2 tied options in `runoff_choices`, and resets `phase_started_at` for binary runoff.

### Cleanup
- `close_dispute`: Closes settled `Dispute` and associated `VoteRecord` accounts, returning rent strictly to each account's `rent_payer`.

---

## 5. Verification Plan
- `pactum-escrow`: `NO_DNA=1 anchor build -p pactum_escrow --ignore-keys`, `cargo test -p pactum-escrow --all-targets`, `cargo clippy`, `cargo +nightly fmt --all -- --check`.
- `pactum-disputes`: `NO_DNA=1 anchor build -p pactum_disputes --ignore-keys`, `cargo test -p pactum-disputes --all-targets`, workspace `cargo clippy --all-targets --tests -- -Dwarnings`, `cargo +nightly fmt --all -- --check`, and `cargo deny check advisories`.
