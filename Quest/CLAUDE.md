# Quest -- Gamification & Progression

The guided path. Quest makes sovereignty engaging, not intimidating. Missions, achievements, skill trees, challenges, cooperative activities, consortia competitions, and progression -- all designed to reward meaningful participation, not engagement metrics or addiction loops. The Covenant governs: no dark patterns, no punishing rest, opt-in everything.

## Source Layout

```
Quest/
  Cargo.toml
  CLAUDE.md
  src/
    lib.rs              -- module declarations + re-exports
    error.rs            -- QuestError enum
    config.rs           -- QuestConfig (presets: casual, standard, ambitious)
    reward.rs           -- RewardType, Badge, BadgeTier, RewardLedger, RewardSource
    achievement.rs      -- Achievement, AchievementCriteria trait, AchievementRegistry
    progression.rs      -- Progression, SkillTree, Streak, FlowCalibration, Difficulty
    mission.rs          -- Mission, MissionEngine, MissionProgress, Objective
    challenge.rs        -- Challenge, ChallengeBoard, ChallengeEntry, ChallengeParticipant
    consortia.rs        -- ConsortiaLeaderboard, MarketCompetition, InnovationQuest
    cooperative.rs      -- CooperativeBoard, GroupAchievement, CooperativeRaid, Mentorship
    engine.rs           -- QuestEngine (central coordinator), QuestStatus, QuestSummary
    observatory.rs      -- QuestObservatory, ObservatoryReport (deidentified aggregate metrics)
```

## Architecture

```
QuestEngine (central coordinator)
    |-- config: QuestConfig
    |-- achievements: AchievementRegistry
    |   +-- AchievementCriteria trait (plugin point)
    |   +-- Built-in: CounterCriteria, FlagCriteria, TimestampCriteria, CompositeCriteria
    |-- missions: MissionEngine
    |   +-- Mission definitions + per-actor MissionProgress
    |   +-- Categories: Onboarding, Daily, Weekly, Seasonal, Personal, Community, Program, Discovery
    |-- challenges: ChallengeBoard
    |   +-- Time-scoped activities, individual or collective scope
    |   +-- Types: Creative, Community, Innovation, Seasonal, Cooperative, Mentorship, Governance
    |-- consortia: ConsortiaLeaderboard
    |   +-- MarketCompetition (weighted metric standings)
    |   +-- SponsoredChallenge (Cool pool funding)
    |   +-- InnovationQuest (themed build competitions)
    |-- cooperative: CooperativeBoard
    |   +-- GroupAchievement, CommunityMilestone, CooperativeRaid, MentorshipProgram
    |-- progressions: HashMap<String, Progression>
    |   +-- XP, level, skill trees, personal bests, streaks
    |-- calibrations: HashMap<String, FlowCalibration>
    |   +-- Adaptive difficulty (Gentle/Normal/Ambitious/Heroic)
    +-- rewards: RewardLedger
        +-- Cool currency, Badges, Unlocks, Titles, SkillPoints

QuestObservatory (deidentified aggregate metrics for Undercroft)
    +-- observe(&QuestEngine) -> ObservatoryReport
    +-- ALL data is aggregate: counts, rates, distributions
    +-- NO pubkeys, NO actor names, NO individual activity
```

## Key Types

- **QuestConfig** -- All tunable parameters. Presets: casual(), standard(), ambitious(). consent_required defaults to true.
- **QuestEngine** -- Central coordinator owning all subsystems. Unified API for XP, achievements, missions, challenges, rewards.
- **QuestStatus** -- Per-actor snapshot (level, XP, missions, achievements, badges, streak, suggested difficulty).
- **QuestSummary** -- Aggregate stats across all actors.
- **Achievement** -- Defined by communities or platform. AchievementCriteria trait for custom evaluation.
- **Mission** -- Multi-step guided experience with objectives, time limits, difficulty, categories.
- **Challenge** -- Time-scoped community/global events. Cooperative > competitive.
- **Progression** -- XP, levels, skill trees, personal bests, streaks with forgiveness.
- **FlowCalibration** -- Adaptive difficulty that keeps participants in flow state.
- **Reward** -- Cool currency, Badges, Unlocks, Titles, SkillPoints.
- **QuestObservatory** -- Generates deidentified ObservatoryReport for Undercroft.
- **ObservatoryReport** -- Aggregate metrics: participation, achievements, missions, challenges, raids, economy.

## Dependencies

```toml
serde = "1"        # serialization
serde_json = "1"   # JSON
thiserror = "2"    # error types
uuid = "1"         # unique IDs
chrono = "0.4"     # timestamps
log = "0.4"        # structured logging
```

**Zero async.** Quest is pure data structures and logic. No platform dependencies. Integration with other crates (Fortune for Cool, Yoke for history, Oracle for guidance) happens at the app layer or through the QuestEngine API.

## Design Principles

- **No dark patterns.** Streaks have forgiveness (configurable grace days). Missing days doesn't punish. The Covenant governs.
- **Opt-in everything.** consent_required is true by default. Quest is a lens, not a gate.
- **Personal bests, not leaderboards.** You compete against yourself.
- **Cooperative > competitive.** Group achievements, raids, and mentorship reward collaboration.
- **Trait-based extensibility.** AchievementCriteria is the plugin point. Communities define their own.
- **No punitive mechanics.** Expired missions can be restarted. Raid defeat = "we'll get 'em next time."

## Covenant Alignment

**Dignity** -- gamification serves the person's growth, never the platform's metrics. No dark patterns, no artificial scarcity, no shame mechanics.
**Sovereignty** -- Quest is optional; everything it rewards is also achievable without it. Progress belongs to the person.
**Consent** -- participation is voluntary, progress is private unless shared. consent_required = true by default.
