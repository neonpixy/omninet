use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::pressure::PressureEvent;

/// A command that modifies the advisor's own cognitive state.
///
/// From Solas v3's bidirectional control: thoughts can issue commands
/// that modify the system's emotional/instinctual state.
/// This creates a true feedback loop — thinking influences feeling,
/// which influences future thinking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StateCommand {
    /// Adjust expression pressure (e.g., after discovering something interesting)
    AdjustPressure(PressureEvent),
    /// Shift what the advisor is paying attention to
    ShiftFocus(Vec<String>),
    /// Change the cognitive mode
    SetMode(CognitiveMode),
    /// Strengthen a specific synapse (reinforcing a connection)
    StrengthenSynapse(Uuid),
    /// Add something to working memory (clipboard)
    ClipboardAdd(String),
    /// Custom state change (extensible)
    Custom {
        key: String,
        value: x::Value,
    },
}

/// Whether the advisor is reactive or proactive.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum CognitiveMode {
    /// Reactive: waits for user input, only responds when asked
    #[default]
    Assistant,
    /// Proactive: generates autonomous thoughts, speaks when compelled
    Autonomous,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cognitive_mode_default() {
        assert_eq!(CognitiveMode::default(), CognitiveMode::Assistant);
    }

    #[test]
    fn state_command_variants() {
        let cmds: Vec<StateCommand> = vec![
            StateCommand::AdjustPressure(PressureEvent::NovelContent),
            StateCommand::ShiftFocus(vec!["design".into()]),
            StateCommand::SetMode(CognitiveMode::Autonomous),
            StateCommand::StrengthenSynapse(Uuid::new_v4()),
            StateCommand::ClipboardAdd("insight".into()),
            StateCommand::Custom {
                key: "energy".into(),
                value: x::Value::Double(0.8),
            },
        ];
        assert_eq!(cmds.len(), 6);
    }

    #[test]
    fn state_command_serialization() {
        let cmd = StateCommand::ShiftFocus(vec!["rust".into(), "design".into()]);
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: StateCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }
}
