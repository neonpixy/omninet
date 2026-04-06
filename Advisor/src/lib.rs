//! # Advisor — AI Cognition for Omnidea
//!
//! The wise counselor. Advisor is a continuously thinking mind, not a chatbot.
//! It processes thoughts autonomously, builds connections via synapses, and only
//! speaks when it has something worth saying (or when asked).
//!
//! AI companions are first-class citizens of Omnidea. One companion per person,
//! sponsored by a human, bound by the Covenant like everyone else.
//!
//! # Architecture
//!
//! - **Thought Layer** — autonomous impulses with lifecycle tracking
//! - **Synapse Layer** — weighted cognitive graph connecting entities
//! - **Pressure Layer** — urge-to-speak accumulator that gates expression
//! - **Engine Layer** — pluggable LLM backends (Claude, local models)
//! - **Store Layer** — in-memory cognitive state with embedding search
//! - **Skill Layer** — tool calling across 7 Throne programs
//! - **Cognitive Loop** — sync state machine driven by the platform layer
//! - **Sacred Layer** — Covenant constraints (sponsorship, consent, audit)
//! - **Governance** — Liquid Democracy delegation for community votes
//! - **Capability Floor** — minimum AI requirements for governance participation
//! - **Consent Escalation** — granular gates for Advisor actions

pub mod bridge;
pub mod capability_floor;
pub mod cognitive_loop;
pub mod config;
pub mod consent_escalation;
pub mod engine;
pub mod error;
pub mod federation_scope;
pub mod governance;
pub mod pressure;
pub mod sacred;
pub mod skill;
pub mod store;
pub mod synapse;
pub mod thought;

// ── Re-exports ───────────────────────────────────────────────────

// Error
pub use error::AdvisorError;

// Config
pub use config::AdvisorConfig;

// Sacred (Covenant constraints)
pub use sacred::{AuditRecord, ConsentLevel, ExpressionConsent, SponsorshipBond};

// Thought
pub use thought::{
    ExternalThought, Session, SessionSummary, SessionType, Thought, ThoughtChunk,
    ThoughtPriority, ThoughtSource,
};

// Synapse
pub use synapse::{CustomRelationship, EntityType, RelationshipType, Synapse, SynapseQuery};

// Pressure
pub use pressure::{ExpressionPressure, PressureConfig, PressureEvent, PressureSnapshot};

// Engine
pub use engine::{
    ClaudeContent, ClaudeContentBlock, ClaudeMessage, ClaudeProvider, ClaudeRequest,
    ClaudeResponse, ClaudeResponseBlock, ClaudeTool, ClaudeUsage, CognitiveProvider,
    ConversationMessage, FinishReason, GenerationContext, GenerationResult, LocalProvider,
    MessageRole, ProviderCapabilities, ProviderInfo, ProviderPreferences, ProviderRegistry,
    ProviderRouter, ProviderStatus, SecurityTier, SelectionStrategy,
};

// Store
pub use store::{
    ClipboardEntry, CognitiveStore, CognitiveStoreState, EmbeddingProvider, GlobalClipboard,
    Memory, MemoryResult, TfIdfProvider, cosine_similarity,
};

// Skill
pub use skill::{
    SkillCall, SkillCategory, SkillDefinition, SkillParameter, SkillRegistry, SkillResult,
    SkillValidationResult,
};

// Cognitive Loop
pub use cognitive_loop::{
    CognitiveAction, CognitiveEvent, CognitiveLoop, CognitiveMode, InnerThought, InnerVoice,
    StateCommand,
};

// Bridge
pub use bridge::{ActionBridge, BridgeOutput, BridgeRegistry};

// Governance
pub use governance::{
    GovernanceAIPolicy, GovernanceAction, GovernanceMode, GovernanceModeConfig, GovernanceVote,
    OverrideSignal, ProposalAnalysis, ProposalType, ReasoningDetail, ReasoningTransparency,
    ValueProfile, VotePosition, VotingPattern,
};

// Capability Floor (R6A)
pub use capability_floor::{
    BenchmarkResult, CapabilityAssessment, CapabilityBenchmark, DeferToHuman,
    MinimumCapabilities, should_defer_governance,
};

// Consent Escalation (R6D)
pub use consent_escalation::{
    CommunityConsentPolicy, ConsentApproval, ConsentEscalation, ConsentGate, ConsentProfile,
    PendingAction,
};

// Federation Scope
pub use federation_scope::FederationScope;
