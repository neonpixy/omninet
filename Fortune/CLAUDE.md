# Fortune -- Economic Primitives

Regenerative economics. Capacity-backed supply, UBI, demurrage, bearer cash, cooperative structures, transparent ledger, progressive redistribution. The treasury of a world that doesn't hoard.

## Architecture

### Source Layout

```
Fortune/src/
  lib.rs              -- module declarations + re-exports
  error.rs            -- FortuneError enum (Clone + PartialEq)
  policy.rs           -- FortunePolicy (configurable params, presets)
  treasury.rs         -- Treasury (capacity-backed supply), NetworkMetrics, MintRecord, TreasuryStatus
  balance.rs          -- Balance (liquid + locked), Ledger, Transaction, TransactionSummary, TransactionType, TransactionReason
  ubi.rs              -- UbiDistributor (eligibility pipeline), ClaimRecord, UbiEligibility, IneligibilityReason
  demurrage.rs        -- DemurrageEngine (compound daily decay), DemurragePreview, DemurrageCycleResult
  cash/
    mod.rs            -- submodule re-exports
    note.rs           -- CashNote, serial generation (31-char alphabet), validation
    mint.rs           -- CashMint (issuance + rate limiting)
    registry.rs       -- CashRegistry (lifecycle tracking), CashStatus
    redemption.rs     -- CashRedemption (validation + unlock flow), RedemptionResult
  exchange.rs         -- TradeProposal, EscrowRecord, Exchange trait, TradeStatus, EscrowStatus, ReleaseCondition
  flow_back.rs        -- FlowBack (progressive marginal redistribution), FlowBackPreview
  cooperative.rs      -- Cooperative, CooperativeMember, CooperativeStatus, SurplusDistribution (4 models)
  trust.rs            -- CommonsTrust, TrustType (5), StewardshipRecord, TrustAsset
  pattern_detection.rs -- R2F: TransactionTier(Private/Receipted/Approved), TransactionTierPolicy, TransactionReceipt, ApprovalRequest, FinancialPatternDetector (4 detectors), FinancialAlert
```

### Key Types

- **FortunePolicy** -- Configurable economic parameters. Fields for UBI amount, UBI period, demurrage rate, balance cap for UBI eligibility, flow-back tiers, cash expiry, etc. Presets: `default()`, `testing()`, `conservative()`.
- **Treasury** -- Capacity-backed supply. Tracks NetworkMetrics (users, ideas, collectives) to compute max supply. MintRecord logs every issuance. TreasuryStatus reports minted vs available.
- **Balance** -- liquid + locked amounts. Ledger holds balances by owner ID with full Transaction history.
- **Transaction** -- from, to, amount, transaction_type (Credit/Debit/Lock/Unlock), reason (enum: Mint, Ubi, Transfer, Demurrage, FlowBack, CashIssue, CashRedeem, EscrowLock, EscrowRelease, CoopDistribution, TrustAllocation), timestamp.
- **UbiDistributor** -- Eligibility pipeline: checks verified identity, balance cap, cooldown period, active enactment, not suspended, within supply. ClaimRecord tracks claims.
- **DemurrageEngine** -- Compound daily decay. Formula: `decay = balance - balance * (1 - rate/30)^days`. DemurragePreview shows projected decay. DemurrageCycleResult captures actual amounts.
- **CashNote** -- Bearer instrument. Serial format: XXXX-XXXX-XXXX (31-char unambiguous alphabet, excludes 0/O/I/L/1). Fields: serial, denomination, issuer, issued_at, expires_at, redeemed status.
- **CashMint** -- Issues notes with rate limiting per issuer.
- **CashRegistry** -- Tracks note lifecycle. CashStatus: Active, Redeemed, Expired, Revoked.
- **CashRedemption** -- Validates serial, checks expiry/revocation, returns RedemptionResult.
- **TradeProposal** -- Describes an exchange offer. EscrowRecord locks funds until conditions met.
- **Exchange** -- Trait for trade execution.
- **FlowBack** -- Progressive marginal redistribution. Tiers configured via FlowBackTier in FortunePolicy. FlowBackPreview shows breakdown by tier.
- **Cooperative** -- Members, surplus distribution model (Equal, ProportionalToContribution, NeedsBased, Hybrid). CooperativeStatus lifecycle.
- **CommonsTrust** -- Stewardship over shared resources. TrustType: Land, Water, Forest, Digital, Cultural. StewardshipRecord tracks steward actions.

### Key Formulas

| Component | Formula |
|-----------|---------|
| **Max Supply** | `(users * 1000) + (ideas * 10) + (collectives * 5000)` |
| **Demurrage** | `decay = balance - balance * (1 - rate/30)^days` |
| **UBI Cap** | Balance must be < 500 Cool to claim (default policy) |
| **Flow-back** | Marginal: 1% above 1M, 3% above 10M, 5% above 100M, 7% above 1B (default policy) |
| **Cash Serial** | XXXX-XXXX-XXXX (31-char unambiguous alphabet) |

### Covenant Sources

| Module | Covenant Source |
|--------|---------------|
| treasury.rs | Consortium Art. 2 SS1 (regenerative commerce) |
| ubi.rs | Conjunction Art. 7 SS2 (livelihood as birthright) |
| demurrage.rs | Consortium Art. 2 SS3 (surplus circulation) |
| cash/ | Consortium Art. 2 SS2 (fair compensation, transparency) |
| exchange.rs | Consortium Art. 2 SS2 (mutual benefit, just exchange) |
| flow_back.rs | Conjunction Art. 6 SS3 (redistribution of excess) |
| cooperative.rs | Conjunction Art. 7 SS6 (cooperative economics) |
| trust.rs | Conjunction Art. 6 SS1 (stewardship over ownership) |

## Dependencies

```toml
x = { path = "../X" }         # Value type
ideas = { path = "../Ideas" }  # Coinage (Cool type)
crown = { path = "../Crown" }  # Identity/signatures
serde, serde_json, thiserror, uuid, chrono, log, getrandom
```

**Zero async.** Fortune is pure data structures and logic. FortuneError is Clone + PartialEq.

### Financial Pattern Detection (`pattern_detection.rs`) — R2F

Community-governed financial accountability. Not surveillance -- pattern detection that flags potential abuse for community governance review. Communities opt into transparency by setting a policy in their charter. No community = no policy = all private.

**Transaction Tiers:**

| Tier | Threshold | Visibility |
|------|-----------|-----------|
| **Private** | <= 500 Cool (default) | Buyer + seller only. Invisible to governance. |
| **Receipted** | 501 -- 10,000 Cool (default) | Receipt visible to governance: amount, parties, timestamp. NOT content/purpose. |
| **Approved** | > 10,000 Cool (default) | Requires multi-sig from governance before execution. |

**Key Types:**
- **TransactionTierPolicy** -- Community-configured thresholds. Fields: `private_ceiling`, `receipted_ceiling`, `approved_floor`, `cash_note_max_denomination`, `policy_applies_to` (TierScope: AllTransactions, CommunityTransactionsOnly, InterCommunityOnly). `classify(amount) -> TransactionTier`.
- **TransactionReceipt** -- Receipt for a receipted/approved transaction: id, from_pubkey, to_pubkey, amount, tier, timestamp, community_id. Governance sees THAT a transaction happened, never WHY.
- **ApprovalRequest** -- Multi-sig approval for Approved-tier transactions. `add_signature()` validates: not already signed, not expired, still pending. `evaluate()` re-checks threshold. Statuses: Pending, Approved, Executed, Rejected, Expired.
- **FinancialPatternDetector** -- Stateful detector with configurable `DetectorConfig`. Four detection methods:
  1. `detect_structuring()` -- transactions systematically just below the receipted ceiling (sliding window, configurable proximity %).
  2. `detect_rapid_cash_cycling()` -- CashNotes minted and redeemed in quick succession (configurable min gap).
  3. `detect_circular_flow()` -- A -> B -> C -> A with net transfer near zero (3-node cycle detection, configurable tolerance).
  4. `detect_volume_anomaly()` -- sudden spike >3 std devs from rolling average (configurable window and period).
- **FinancialAlert** -- Generated alert: pattern type, pubkeys involved, community_id, severity (Info/Warning/Escalation), auto_action (NotifyGovernance/HoldTransaction/RequireAdditionalApproval).
- **DetectorConfig** -- All sensitivity knobs: structuring_count_threshold (5), structuring_proximity_pct (0.10), structuring_window_secs (86400), cash_cycling_min_gap_secs (3600), volume_anomaly_std_devs (3.0), volume_rolling_window (30), volume_period_secs (86400).

**Design principle:** No `BlockTransaction` action exists. Communities decide, not algorithms. Alerts go to governance; governance responds.

**Covenant source:** Consortium Art. 2 SS5 (economic transparency and public accountability).

## What Does NOT Live Here

- **Constitutional enforcement** -- Polity (P)
- **Community governance** -- Kingdom (K)
- **Safety mechanisms** -- Bulwark (B)
- **Accountability** -- Jail (J)
- **Encrypted storage** -- Sentinal/Vault
- **Inter-module communication** -- Equipment (Pact)

Fortune defines the economic atoms. Communities (via Kingdom) configure policies within Covenant bounds.

## Covenant Alignment

**Dignity** -- UBI provides a floor; every verified person receives Cool as a birthright.
**Sovereignty** -- communities set their own economic parameters within Covenant bounds.
**Consent** -- all exchange is voluntary; accumulation is bounded by flow-back.
