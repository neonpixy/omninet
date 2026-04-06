use serde::{Deserialize, Serialize};

use super::command::StateCommand;
use crate::engine::GenerationContext;
use crate::thought::Thought;

/// What the cognitive loop wants the caller to do.
///
/// The loop is a pure state machine — it doesn't own timers or make network calls.
/// Instead, it returns actions that the platform layer (Divinity/Apple) executes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CognitiveAction {
    /// Request an LLM generation (caller sends to provider, feeds result back)
    RequestGeneration(GenerationContext),
    /// Express a thought to the user
    Express(Thought),
    /// Store a thought internally (home session monologue, not shown to user)
    Store(Thought),
    /// Modify the cognitive state (bidirectional feedback)
    ModifyState(StateCommand),
    /// Emit a cognitive event (for logging/observability)
    Emit(CognitiveEvent),
}

/// Observable events from the cognitive loop.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CognitiveEvent {
    /// A tick completed
    TickCompleted {
        pressure: f64,
        mode: String,
    },
    /// Expression pressure crossed the threshold
    PressureThresholdReached {
        pressure: f64,
        threshold: f64,
    },
    /// The advisor woke up (entered autonomous mode)
    Awakened,
    /// The advisor went to sleep (entered assistant mode)
    Asleep,
    /// Inner voice generated a thought
    InnerVoiceThought {
        summary: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cognitive_event_serialization() {
        let event = CognitiveEvent::PressureThresholdReached {
            pressure: 0.85,
            threshold: 0.8,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: CognitiveEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn all_event_variants() {
        let events = [
            CognitiveEvent::TickCompleted { pressure: 0.5, mode: "autonomous".into() },
            CognitiveEvent::PressureThresholdReached { pressure: 0.85, threshold: 0.8 },
            CognitiveEvent::Awakened,
            CognitiveEvent::Asleep,
            CognitiveEvent::InnerVoiceThought { summary: "something".into() },
        ];
        assert_eq!(events.len(), 5);
    }
}
