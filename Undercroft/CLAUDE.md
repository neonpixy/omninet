# Undercroft -- The Vaulted Chamber Beneath the Castle

The meta-layer. Undercroft is the one crate that can depend on everything above it. It observes, never controls. System health, network topology, economic vitals -- everything that requires a god view of Omninet lives here.

HQ is the app that peers into the Undercroft.

## Source Layout

```
Undercroft/
  Cargo.toml
  CLAUDE.md
  src/
    lib.rs              -- module declarations + re-exports
    error.rs            -- UndercraftError enum
    network.rs          -- NetworkHealth (aggregated relay health from Globe)
    community.rs        -- CommunityHealth + GovernanceActivity (from Kingdom + Bulwark)
    economic.rs         -- EconomicHealth (from Fortune's TreasuryStatus)
    quest.rs            -- QuestHealth (from Quest's ObservatoryReport)
    snapshot.rs         -- HealthSnapshot + HealthHistory (ring buffer)
    metrics.rs          -- HealthMetrics (top-level composite for HQ dashboard)
  AppCatalog/           -- sub-crate for app/extension catalog
  DeviceManager/        -- sub-crate for device management
```

## Architecture

```
Undercroft (health aggregation)
    |-- NetworkHealth: aggregate relay counts, scores, latency
    |   +-- from_relay_health(&[globe::RelayHealth]) -> Self
    |-- CommunityHealth: per-community member/role/governance counts
    |   +-- from_community(&Community, &[Proposal], Option<&CollectiveHealthPulse>) -> Self
    |-- EconomicHealth: treasury supply, circulation, utilization
    |   +-- from_treasury_status(&TreasuryStatus) -> Self
    |-- QuestHealth: deidentified Quest aggregate metrics
    |   +-- from_report(&ObservatoryReport) -> Self
    |   +-- health_score() -> f64 (composite 0.0-1.0)
    |-- HealthSnapshot: {network, communities, economic, quest?, timestamp}
    |-- HealthHistory: VecDeque ring buffer (default 168 = 1 week hourly)
    |   +-- push(), latest(), iter(), len(), is_empty()
    +-- HealthMetrics: top-level summary (node/relay/event/cool/community counts)
        +-- from_snapshot(&HealthSnapshot, node_count, Option<&StoreStats>) -> Self
```

## Key Types

- **NetworkHealth** -- Aggregated relay health. relay_count, connected_count, average_score, total_send/receive/error, average_latency_ms. NO relay URLs stored.
- **CommunityHealth** -- Per-community metrics. member_count, active_status, role_distribution (role name -> count, NOT pubkeys), GovernanceActivity. NO member identities stored.
- **GovernanceActivity** -- active_proposals, resolved_proposals, total_votes_cast, average_participation. NO voter identities.
- **EconomicHealth** -- Treasury vitals. max_supply, in_circulation, locked_in_cash, available, utilization, active_users/ideas/collectives counts.
- **QuestHealth** -- Deidentified Quest metrics. Wraps ObservatoryReport. Provides health_score() (0.0-1.0) for Omny/Home.
- **HealthSnapshot** -- Complete point-in-time snapshot: network + communities + economic + quest? + timestamp.
- **HealthHistory** -- Ring buffer of HealthSnapshots. Evicts oldest when at capacity.
- **HealthMetrics** -- Dashboard summary: node_count, relay_count, event_throughput, content_volume, cool_circulation, community_count, quest_participants, quest_health_score, active_challenges, active_raids.
- **UndercraftError** -- NoData, CommunityNotFound, CapacityExceeded.

## Covenant Constraints (CRITICAL)

ALL metrics are DEIDENTIFIED. This is non-negotiable.

- **NO pubkeys** -- never stored, never serialized, never logged.
- **NO individual activity** -- only aggregate counts and rates.
- **NO relay URLs** -- only relay counts and average scores.
- **NO location data** -- no geographic information of any kind.
- **Aggregated** -- "10,000 transactions today" not "Alice sent Bob 5 Cool."
- **Read-only** -- observes, never controls. No kill switches, no overrides.
- **Transparent** -- open-source and auditable. Opacity is breach.

## Dependencies

```toml
globe = { path = "../Globe" }       # RelayHealth, StoreStats, ConnectionState
kingdom = { path = "../Kingdom" }   # Community, Proposal, CommunityRole, ProposalStatus
fortune = { path = "../Fortune" }   # TreasuryStatus, NetworkMetrics
bulwark = { path = "../Bulwark" }   # CollectiveHealthPulse, CollectiveHealthStatus
quest = { path = "../Quest" }       # ObservatoryReport (deidentified Quest metrics)
serde, serde_json, thiserror, uuid, chrono, log
```

**Zero async.** Undercroft is pure data structures and aggregation logic.

## The Greek Alphabet (Subsystems)

24 Greek letters, 24 possible subsystems. Only build what's needed.

| Letter | Name | What It Does |
|--------|------|-------------|
| A | **Alpha** | System health. Aggregate metrics across all 26 ABCs. The vital signs. (IMPLEMENTED as Health Aggregation) |
| B | **Beta** | Testing and staging. The proving ground. |
| G | **Gamma** | Network observatory. Relay topology, latency mapping. |
| D | **Delta** | Economic observatory. Cool circulation velocity, treasury health. |
| ... | *(18 more available)* | Built as needs emerge. The alphabet has room. |
| O | **Omega** | Operator console. Unified interface to all Greek subsystems. |

## Covenant Alignment

**Dignity** -- health metrics protect communities without surveilling individuals.
**Sovereignty** -- every person's data remains their own; only aggregate signals flow here.
**Consent** -- the observatory is transparent and auditable; opacity is breach.
