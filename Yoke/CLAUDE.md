# Yoke — History & Provenance

The binding thread. Yoke remembers. Version history, provenance chains, collective memory, and the ceremonial record. We are yoked to our history — not as a burden, but as a foundation.

## Architecture

Yoke is pure data structures and logic. Zero async, zero platform dependencies. It defines the vocabulary and graph that apps use to track history and provenance. Globe events carry Yoke data across the network (kinds 25000-25999).

## Key Types

### Typed Relationships (`relationship.rs`)

A vocabulary of edges connecting events, ideas, and people:

- **DerivedFrom** — creative lineage (this was made from that)
- **VersionOf** — version chain (this is a version of that)
- **ApprovedBy** — governance link (this was approved by that proposal/vote)
- **CommentOn** — social (this comments on that)
- **Supersedes** — replacement (this replaces that)
- **References** — citation (this mentions that)
- **BranchedFrom** — exploration (this was branched from that)
- **MergedInto** — reunion (this was merged into that)
- **RespondsTo** — reply (this responds to that)
- **Endorses** — recommendation (this endorses that)
- **Amends** — constitutional (this amends that)
- **Custom(String)** — app-defined

Categories: `is_provenance()` (DerivedFrom, VersionOf, BranchedFrom, MergedInto, Amends), `is_social()` (ApprovedBy, CommentOn, RespondsTo, Endorses), `is_structural()` (Supersedes, References).

**`YokeLink`** — a typed edge with source, target, RelationType, author, timestamp, and optional metadata HashMap.

### Version History (`version.rs`)

- **`VersionTag`** — named snapshot of an .idea at a VectorClock point ("v2.0", "approved-final"). Builder methods: `on_branch()`, `with_message()`.
- **`VersionChain`** — complete version history per idea: tags, branches, merges. API: `tag_version()`, `create_branch()`, `merge_branch()`, `versions_on_branch()`, `latest_version()`, `is_branch_merged()`.
- **`Branch`** — fork a timeline for exploration (dark-mode, rebrand, etc.)
- **`MergeRecord`** — join branches back together
- Validates: no duplicate names on same branch, no branch named "main", no branching from nonexistent versions, no double merges

### Activity Timeline (`timeline.rs`)

- **`ActivityRecord`** — who did what to what, when (actor + action + target + context). Builder: `with_context()`, `in_community()`.
- **`ActivityAction`** — 14 built-in + Custom: Created, Updated, Deleted, Approved, Rejected, Commented, Shared, Transferred, Branched, Merged, Tagged, Published, Endorsed, Flagged.
- **`TargetType`** — what was acted on: Event, Idea, Community, Person, Proposal, Asset, Custom.
- **`Timeline`** — capacity-managed activity stream with configurable `TimelineConfig`.
- **`TimelineConfig`** — `max_activities` (default 10K, oldest evicted), `max_milestones` (default 1K).
- **`Milestone`** — named moment with significance (Minor, Notable, Major, Historic), community context, related events. Builder: `with_description()`, `in_community()`, `with_related_event()`.
- Queries: `by_actor()`, `by_action()`, `for_target()`, `in_community()`, `between(since, until)`, `milestones_at_least()`.
- `prune_before(cutoff)` — remove activities older than a timestamp.

### Relationship Graph (`graph.rs`)

- **`RelationshipGraph`** — dual adjacency lists (forward + reverse) for O(1) lookups
- `add_link()`, `links_from()`, `links_to()` — basic operations
- **`ancestors()`** — follow provenance links backward (DerivedFrom chain)
- **`descendants()`** — follow provenance links forward (what derived from this)
- **`traverse_by_type()`** — BFS following only a specific relationship type
- **`versions_of()`**, **`comments_on()`**, **`endorsements_of()`**, **`superseded_by()`** — typed convenience queries
- **`links_from_by_author()`**, **`links_to_by_author()`** — filter by who created the link
- **`links_from_between()`**, **`links_to_between()`** — filter by time range
- **`path_between()`** — BFS shortest path (undirected, both forward and reverse)
- **`remove_entity()`** — clean removal from both adjacency lists
- **`snapshot()` / `from_snapshot()`** — serialize/restore via `GraphSnapshot`
- `Direction` enum (Forward/Backward). `TraversalNode` — entity_id + depth + path.
- Cycle-safe (visited set prevents infinite loops)

### Ceremonies (`ceremony.rs`)

- **`CeremonyRecord`** — the moments that matter, with **validation**. Builder: `with_principal()`, `with_witness()`, `with_officiant()`, `in_community()`, `with_content()`, `with_related_event()`.
- **`CeremonyType`** — CovenantOath, CommunityFormation, UnionFormation, CharterAmendment, Dissolution, LeadershipTransition, ConstitutionalReview, Custom(String).
- **`ParticipantRole`** — Principal, Witness, Officiant, Custom.
- **`CeremonyParticipant`** — pubkey + role.
- **`validate()`** enforces structural rules:
  - CovenantOath: at least 1 principal
  - CommunityFormation: at least 1 principal + community_id
  - UnionFormation: at least 2 principals
  - Dissolution: at least 1 principal + community_id
  - LeadershipTransition: at least 1 principal + community_id
  - CharterAmendment/ConstitutionalReview: community_id
  - Custom: no requirements
- Convenience queries: `principals()`, `witnesses()`, `officiants()`, `participant_count()`.

### Builder (`builder.rs`)

Tag construction helpers producing the exact tag structure Globe's filters expect:
- `relationship_tags()`, `version_tag_tags()`, `branch_tags()`, `merge_tags()`
- `milestone_tags()`, `ceremony_tags()`, `activity_tags()`
- Content serialization: `relationship_content()`, `version_tag_content()`, etc.

### Provenance Scoring (`provenance.rs`) — R4B

Structural transparency for information. Not fact-checking, not content moderation, not a trust score on people. Source transparency, chain of custody, corroboration visibility, challenge visibility.

- **ProvenanceScore** -- Computed score (0.0 to 1.0) for a single event. `is_strong()` >= 0.7, `is_weak()` < 0.3. Composed from `ProvenanceFactors`.
- **ProvenanceFactors** -- 8 weighted dimensions:
  - `source_identified` (0.20) -- is the original creator identifiable?
  - `source_reputation` (0.15) -- Bulwark reputation of the original creator
  - `corroboration_count` (0.20) -- independent sources with similar content (log scale, caps at 10)
  - `corroboration_diversity` (0.15) -- Simpson's index across corroborating communities
  - `age_days` (0.05) -- older = more time for corroboration/refutation (caps at 365)
  - `modification_chain_length` (0.10) -- shorter = better (inverse, caps at 10)
  - `challenge_count` (0.15) -- no challenges = full score, more = lower (1/(1+n))
- **ProvenanceChain** -- Full chain from current event to original: `original_event_id`, `original_author`, `chain` (Vec<ProvenanceLink>), `corroborations`. `has_known_origin()`, `chain_depth()`, `corroborating_communities()`.
- **ProvenanceLink** -- Single link in the chain: event_id, author, RelationType, timestamp.
- **Corroboration** -- Independent confirmation: event_id, author, community_id, similarity (0.0-1.0).
- **ProvenanceComputer** -- Stateless. `compute()` traces the RelationshipGraph to find the original, then computes factors. `trace_chain()` follows DerivedFrom/VersionOf/BranchedFrom backward (cycle-safe). `build_chain()` constructs the full ProvenanceChain.
- **EventData** -- Minimal event metadata (avoids Globe dependency): id, author, community_id, created_at.

### AI Transparency (`ai_provenance.rs`) — R6C

Attribution tracking for AI-assisted actions and content authorship. When Advisor assists, the resulting ActivityRecord includes `AdvisorAttribution`. Any .idea viewer can see: "This design was 78% human-created, 22% AI-assisted." AI assistance is not penalized -- the goal is honest provenance.

- **AdvisorAttribution** -- Per-action AI attribution: `action_type` ("design_suggestion", "text_edit", etc.), `was_accepted`, `was_modified`, `provider_id`, `confidence` (0.0-1.0). Builder: `new().accepted().with_confidence()`. Queries: `is_collaborative()` (accepted + modified), `is_pure_ai()` (accepted, not modified), `is_rejected()`.
- **AuthorshipSource** -- Enum: `Human(pubkey)`, `Advisor(sponsor_pubkey, provider_id)`, `Collaborative(human_pubkey, provider_id)`. `involves_ai()`, `is_human()`, `human_pubkey()`, `provider_id()`.
- **AuthorshipEntry** -- Per-Digit authorship record: `digit_id` (Uuid), `created_by`, `last_modified_by`, `last_modified_at`. `modify()` updates. `has_ai_involvement()` checks both creation and modification.
- **IdeaAuthorship** -- Aggregate for an entire .idea file: `human_actions`, `advisor_actions`, `advisor_percentage` (0.0-100.0), `breakdown` (Vec<AuthorshipEntry>). `record_creation()`, `record_modification()` update counts and recompute percentage. `digits_with_ai()`, `digits_purely_human()`, `from_entries()`.

## Event Kinds (25000-25999)

| Kind | Name | Content | Key Tags |
|------|------|---------|----------|
| 25000 | RELATIONSHIP | JSON YokeLink | source, target, rel |
| 25001 | VERSION_TAG | JSON VersionTag | d (idea_id), branch, version |
| 25002 | BRANCH | JSON Branch | d (idea_id), branch, from |
| 25003 | MERGE | JSON MergeRecord | d (idea_id), source, target |
| 25004 | MILESTONE | JSON Milestone | d (milestone_id), community |
| 25005 | CEREMONY | JSON CeremonyRecord | d (ceremony_id), type, community |
| 25006 | ACTIVITY | JSON ActivityRecord | actor, action, target |

`kind::is_yoke_kind(kind)` checks if a kind is in range 25000-25999.

## Dependencies

```toml
x = { path = "../X" }       # Value type, VectorClock
crown = { path = "../Crown" } # Identity
serde, serde_json, uuid, chrono, thiserror, log
```

**Zero async.** Yoke is pure data structures and logic.

## Covenant Alignment

**Dignity** — history belongs to the people who made it, not a platform that can rewrite it. **Sovereignty** — your history is yours. **Consent** — the duty of remembrance is collective, but individual participation is voluntary.
