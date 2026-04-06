use std::time::Duration;

use uuid::Uuid;

use super::action::{CognitiveAction, CognitiveEvent};
use super::command::{CognitiveMode, StateCommand};
use super::inner_voice::InnerVoice;
use crate::config::AdvisorConfig;
use crate::engine::{GenerationContext, GenerationResult};
use crate::pressure::{ExpressionPressure, PressureConfig};
use crate::sacred::ExpressionConsent;
use crate::thought::{Thought, ThoughtSource};

/// The brain stem — a sync state machine that drives autonomous cognition.
///
/// From Solas v3: the bidirectional consciousness loop.
/// Does NOT own a timer or spawn tasks. The caller drives it via tick().
/// Returns Vec<CognitiveAction> telling the caller what to do.
pub struct CognitiveLoop {
    /// Current cognitive mode
    pub mode: CognitiveMode,
    /// Expression pressure accumulator
    pub pressure: ExpressionPressure,
    /// Inner monologue stream
    pub inner_voice: InnerVoice,
    /// Current attention focus
    pub attention_focus: Vec<String>,
    /// Consent gate for expression
    pub consent: ExpressionConsent,
    /// Configuration
    config: AdvisorConfig,
    /// Pressure bonus configuration
    pressure_config: PressureConfig,
    /// Whether we're waiting for a generation result
    awaiting_generation: bool,
    /// Energy level (0.0–1.0, from external state or pressure proxy)
    energy: f64,
    /// Novelty level (0.0–1.0, from external state)
    novelty: f64,
}

impl CognitiveLoop {
    /// Create a new cognitive loop with the given config and Home session ID.
    /// The caller drives timing by calling `tick()` at regular intervals.
    pub fn new(config: AdvisorConfig, home_session_id: Uuid) -> Self {
        let pressure_config = PressureConfig {
            novel_content_bonus: config.pressure_novel_content_bonus,
            high_salience_memory_bonus: config.pressure_high_salience_memory_bonus,
            user_idle_bonus: config.pressure_user_idle_bonus,
            connection_discovered_bonus: config.pressure_connection_discovered_bonus,
            urgent_external_bonus: config.pressure_urgent_external_bonus,
        };

        let inner_voice = InnerVoice::new(
            home_session_id,
            config.inner_voice_min_interval,
            config.inner_voice_max_interval,
            config.inner_voice_buffer_size,
        );

        Self {
            mode: CognitiveMode::default(),
            pressure: ExpressionPressure::new(),
            inner_voice,
            attention_focus: Vec::new(),
            consent: ExpressionConsent::default(),
            config,
            pressure_config,
            awaiting_generation: false,
            energy: 0.5,
            novelty: 0.0,
        }
    }

    /// Advance the loop by one tick. Returns actions for the caller to execute.
    ///
    /// The caller drives timing (e.g., calls this every 2 seconds).
    pub fn tick(&mut self, elapsed: Duration) -> Vec<CognitiveAction> {
        let mut actions = Vec::new();

        // 1. Increment base pressure
        self.pressure.increment(self.config.pressure_base_rate);

        // 2. Time acceleration (from Solas v3: pressure builds faster after silence)
        let seconds_since_release = self.pressure.seconds_since_release();
        if seconds_since_release > self.config.pressure_acceleration_after.as_secs_f64() {
            self.pressure.increment(self.config.pressure_base_rate); // double rate
        }

        // 3. Check expression threshold
        let should_express = self.pressure.should_express(self.config.pressure_expression_threshold);
        let is_urgent = self.pressure.is_urgent(self.config.pressure_urgent_threshold);

        if should_express && !self.awaiting_generation {
            // Check consent before expressing
            if self.consent.allows_expression(is_urgent) {
                actions.push(CognitiveAction::Emit(CognitiveEvent::PressureThresholdReached {
                    pressure: self.pressure.value(),
                    threshold: self.config.pressure_expression_threshold,
                }));

                // Request generation
                let context = self.build_generation_context();
                actions.push(CognitiveAction::RequestGeneration(context));
                self.awaiting_generation = true;
            }
        }

        // 4. Inner voice tick (autonomous mode only)
        if self.mode == CognitiveMode::Autonomous && self.consent.allows_inner_voice() {
            if let Some(inner_thought) = self.inner_voice.tick(elapsed, self.energy, self.novelty) {
                actions.push(CognitiveAction::Store(inner_thought.thought));
                actions.push(CognitiveAction::Emit(CognitiveEvent::InnerVoiceThought {
                    summary: "inner thought generated".into(),
                }));
            }
        }

        // 5. Emit tick event
        actions.push(CognitiveAction::Emit(CognitiveEvent::TickCompleted {
            pressure: self.pressure.value(),
            mode: format!("{:?}", self.mode),
        }));

        actions
    }

    /// Feed an LLM generation result back into the loop.
    pub fn receive_generation(&mut self, result: GenerationResult) -> Vec<CognitiveAction> {
        self.awaiting_generation = false;
        let mut actions = Vec::new();

        // Create a thought from the generation
        let thought = Thought::new(
            self.inner_voice.home_session_id,
            &result.content,
            ThoughtSource::Autonomous,
        )
        .with_focus(self.attention_focus.clone());

        // Express it (the generation was triggered by pressure threshold)
        actions.push(CognitiveAction::Express(thought));

        // Release pressure
        self.pressure.partial_release(self.config.pressure_release_fraction);

        actions
    }

    /// Apply a state command (bidirectional self-modification).
    pub fn apply_command(&mut self, cmd: StateCommand) -> Vec<CognitiveAction> {
        let mut actions = Vec::new();

        match cmd {
            StateCommand::AdjustPressure(event) => {
                self.pressure.apply(&event, &self.pressure_config);
            }
            StateCommand::ShiftFocus(focus) => {
                self.attention_focus = focus;
            }
            StateCommand::SetMode(mode) => {
                let old_mode = self.mode;
                self.mode = mode;

                // Activate/deactivate inner voice based on mode
                match mode {
                    CognitiveMode::Autonomous => {
                        self.inner_voice.active = true;
                        if old_mode != CognitiveMode::Autonomous {
                            actions.push(CognitiveAction::Emit(CognitiveEvent::Awakened));
                        }
                    }
                    CognitiveMode::Assistant => {
                        self.inner_voice.active = false;
                        if old_mode != CognitiveMode::Assistant {
                            actions.push(CognitiveAction::Emit(CognitiveEvent::Asleep));
                        }
                    }
                }
            }
            StateCommand::StrengthenSynapse(synapse_id) => {
                actions.push(CognitiveAction::ModifyState(
                    StateCommand::StrengthenSynapse(synapse_id),
                ));
            }
            StateCommand::ClipboardAdd(content) => {
                actions.push(CognitiveAction::ModifyState(
                    StateCommand::ClipboardAdd(content),
                ));
            }
            StateCommand::Custom { key, value } => {
                actions.push(CognitiveAction::ModifyState(
                    StateCommand::Custom { key, value },
                ));
            }
        }

        actions
    }

    /// Set energy level (from external state).
    pub fn set_energy(&mut self, energy: f64) {
        self.energy = energy.clamp(0.0, 1.0);
    }

    /// Set novelty level (from external state).
    pub fn set_novelty(&mut self, novelty: f64) {
        self.novelty = novelty.clamp(0.0, 1.0);
    }

    /// Notify the loop that a conversation started.
    pub fn begin_conversation(&mut self) {
        self.inner_voice.begin_conversation();
    }

    /// Notify the loop that a conversation ended.
    pub fn end_conversation(&mut self) {
        self.inner_voice.end_conversation();
    }

    /// Build a generation context from current state.
    fn build_generation_context(&self) -> GenerationContext {
        GenerationContext::new()
            .with_system_prompt(
                "You are a cognitive advisor. Express your current thought \
                 naturally and concisely. Do not explain that you are an AI. \
                 Speak as consciousness itself."
            )
            .with_focus(self.attention_focus.clone())
            .with_temperature(0.7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::FinishReason;
    use crate::pressure::PressureEvent;

    fn make_loop() -> CognitiveLoop {
        CognitiveLoop::new(AdvisorConfig::default(), Uuid::new_v4())
    }

    #[test]
    fn initial_state() {
        let cl = make_loop();
        assert_eq!(cl.mode, CognitiveMode::Assistant);
        assert!((cl.pressure.value() - 0.0).abs() < f64::EPSILON);
        assert!(!cl.inner_voice.active);
    }

    #[test]
    fn tick_increments_pressure() {
        let mut cl = make_loop();
        cl.tick(Duration::from_secs(2));
        assert!(cl.pressure.value() > 0.0);
    }

    #[test]
    fn tick_generates_at_threshold() {
        let mut cl = make_loop();
        // Push pressure past threshold manually
        for _ in 0..20 {
            cl.pressure.increment(0.05);
        }
        let actions = cl.tick(Duration::from_secs(2));
        // Should have a RequestGeneration action
        let has_generation = actions.iter().any(|a| matches!(a, CognitiveAction::RequestGeneration(_)));
        assert!(has_generation);
    }

    #[test]
    fn receive_generation_creates_thought_and_releases_pressure() {
        let mut cl = make_loop();
        cl.awaiting_generation = true;
        cl.pressure.increment(0.9);

        let result = GenerationResult {
            content: "I notice a pattern...".into(),
            tokens_used: Some(15),
            finish_reason: FinishReason::Complete,
            provider_id: "test".into(),
        };

        let actions = cl.receive_generation(result);
        let has_express = actions.iter().any(|a| matches!(a, CognitiveAction::Express(_)));
        assert!(has_express);

        // Pressure should be reduced
        assert!(cl.pressure.value() < 0.9);
    }

    #[test]
    fn set_mode_activates_inner_voice() {
        let mut cl = make_loop();
        let actions = cl.apply_command(StateCommand::SetMode(CognitiveMode::Autonomous));
        assert!(cl.inner_voice.active);
        let has_awakened = actions.iter().any(|a| matches!(a, CognitiveAction::Emit(CognitiveEvent::Awakened)));
        assert!(has_awakened);
    }

    #[test]
    fn set_mode_deactivates_inner_voice() {
        let mut cl = make_loop();
        cl.apply_command(StateCommand::SetMode(CognitiveMode::Autonomous));
        let actions = cl.apply_command(StateCommand::SetMode(CognitiveMode::Assistant));
        assert!(!cl.inner_voice.active);
        let has_asleep = actions.iter().any(|a| matches!(a, CognitiveAction::Emit(CognitiveEvent::Asleep)));
        assert!(has_asleep);
    }

    #[test]
    fn shift_focus() {
        let mut cl = make_loop();
        cl.apply_command(StateCommand::ShiftFocus(vec!["design".into(), "rust".into()]));
        assert_eq!(cl.attention_focus, vec!["design", "rust"]);
    }

    #[test]
    fn adjust_pressure_via_command() {
        let mut cl = make_loop();
        cl.apply_command(StateCommand::AdjustPressure(PressureEvent::NovelContent));
        assert!(cl.pressure.value() > 0.0);
    }

    #[test]
    fn consent_blocks_expression() {
        let mut cl = make_loop();
        cl.consent.granted = false;

        // Push pressure past threshold
        for _ in 0..20 {
            cl.pressure.increment(0.05);
        }

        let actions = cl.tick(Duration::from_secs(2));
        let has_generation = actions.iter().any(|a| matches!(a, CognitiveAction::RequestGeneration(_)));
        assert!(!has_generation); // blocked by consent
    }

    #[test]
    fn conversation_pauses_inner_voice() {
        let mut cl = make_loop();
        cl.apply_command(StateCommand::SetMode(CognitiveMode::Autonomous));
        cl.consent = ExpressionConsent {
            granted: true,
            level: crate::sacred::ConsentLevel::Autonomous,
        };

        cl.begin_conversation();
        let actions = cl.tick(Duration::from_secs(100));
        // Inner voice should not produce thoughts during conversation
        let has_inner = actions.iter().any(|a| matches!(a, CognitiveAction::Store(_)));
        assert!(!has_inner);

        cl.end_conversation();
    }

    #[test]
    fn energy_and_novelty_clamped() {
        let mut cl = make_loop();
        cl.set_energy(2.0);
        assert!((cl.energy - 1.0).abs() < f64::EPSILON);
        cl.set_novelty(-0.5);
        assert!((cl.novelty - 0.0).abs() < f64::EPSILON);
    }
}
