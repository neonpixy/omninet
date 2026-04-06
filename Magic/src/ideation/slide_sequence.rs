//! SlideSequence layout mode — ordered slides with transitions and presenter view.
//!
//! When `LayoutMode::SlideSequence` is active, the document is treated as an
//! ordered sequence of slides (presentation.slide digits). Magic handles the
//! sequence logic (ordering, current index, transitions); Divinity handles the
//! actual transition animation using Regalia's Surge curves.
//!
//! ## Architecture
//!
//! ```text
//! SlideSequenceState ──┬── current_index ──→ which slide is shown
//!                      ├── slide_ids ──────→ ordered list of slide digit IDs
//!                      ├── transition ─────→ how to animate between slides
//!                      └── presenter ──────→ presenter view configuration
//! ```

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Transition effect between slides.
///
/// Maps to Regalia's Surge animation curves. The platform layer (Divinity)
/// reads these specifications and produces the actual visual transition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlideTransition {
    /// Transition type (e.g., "fade", "slide", "push", "dissolve").
    pub effect: TransitionEffect,
    /// Duration in seconds.
    pub duration_secs: f64,
    /// Direction for directional transitions (slide, push).
    pub direction: TransitionDirection,
}

impl Default for SlideTransition {
    fn default() -> Self {
        Self {
            effect: TransitionEffect::Fade,
            duration_secs: 0.5,
            direction: TransitionDirection::Left,
        }
    }
}

/// The visual effect of a slide transition.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransitionEffect {
    /// Cross-dissolve between slides.
    Fade,
    /// Slide the new slide in from a direction.
    Slide,
    /// Push the old slide out while new slides in.
    Push,
    /// Pixel dissolve between slides.
    Dissolve,
    /// No animation — instant switch.
    None,
    /// Custom transition identified by name.
    Custom(String),
}

/// Direction for directional transitions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransitionDirection {
    #[default]
    Left,
    Right,
    Up,
    Down,
}

/// Configuration for presenter view.
///
/// Presenter view shows: current slide, next slide preview, speaker notes,
/// and an elapsed timer. This is displayed on a secondary screen or in a
/// split-panel view during presentations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresenterView {
    /// Whether presenter view is active.
    pub enabled: bool,
    /// Whether to show the next slide preview.
    pub show_next_slide: bool,
    /// Whether to show speaker notes.
    pub show_notes: bool,
    /// Whether to show the elapsed timer.
    pub show_timer: bool,
    /// Elapsed time in seconds since presentation started.
    pub elapsed_secs: f64,
}

impl Default for PresenterView {
    fn default() -> Self {
        Self {
            enabled: false,
            show_next_slide: true,
            show_notes: true,
            show_timer: true,
            elapsed_secs: 0.0,
        }
    }
}

/// State for a slide sequence presentation.
///
/// Manages the ordering of slides, the current position in the sequence,
/// transitions between slides, and presenter view configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlideSequenceState {
    /// Ordered list of slide digit IDs.
    pub slide_ids: Vec<Uuid>,
    /// Index of the currently displayed slide.
    pub current_index: usize,
    /// Default transition between slides (can be overridden per-slide).
    pub default_transition: SlideTransition,
    /// Per-slide transition overrides. Key is the target slide's index.
    pub slide_transitions: std::collections::HashMap<usize, SlideTransition>,
    /// Presenter view configuration.
    pub presenter: PresenterView,
}

impl SlideSequenceState {
    /// Create a new slide sequence with the given slide IDs.
    pub fn new(slide_ids: Vec<Uuid>) -> Self {
        Self {
            slide_ids,
            current_index: 0,
            default_transition: SlideTransition::default(),
            slide_transitions: std::collections::HashMap::new(),
            presenter: PresenterView::default(),
        }
    }

    /// The currently displayed slide ID, if any.
    pub fn current_slide_id(&self) -> Option<Uuid> {
        self.slide_ids.get(self.current_index).copied()
    }

    /// The next slide ID (for presenter preview), if any.
    pub fn next_slide_id(&self) -> Option<Uuid> {
        self.slide_ids.get(self.current_index + 1).copied()
    }

    /// The previous slide ID, if any.
    pub fn previous_slide_id(&self) -> Option<Uuid> {
        if self.current_index > 0 {
            self.slide_ids.get(self.current_index - 1).copied()
        } else {
            None
        }
    }

    /// Total number of slides.
    pub fn slide_count(&self) -> usize {
        self.slide_ids.len()
    }

    /// Whether we're on the first slide.
    pub fn is_first(&self) -> bool {
        self.current_index == 0
    }

    /// Whether we're on the last slide.
    pub fn is_last(&self) -> bool {
        self.current_index + 1 >= self.slide_ids.len()
    }

    /// Move to the next slide. Returns `true` if moved, `false` if already at end.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> bool {
        if self.current_index + 1 < self.slide_ids.len() {
            self.current_index += 1;
            true
        } else {
            false
        }
    }

    /// Move to the previous slide. Returns `true` if moved, `false` if already at start.
    pub fn previous(&mut self) -> bool {
        if self.current_index > 0 {
            self.current_index -= 1;
            true
        } else {
            false
        }
    }

    /// Jump to a specific slide index. Returns `true` if valid, `false` if out of range.
    pub fn go_to(&mut self, index: usize) -> bool {
        if index < self.slide_ids.len() {
            self.current_index = index;
            true
        } else {
            false
        }
    }

    /// Get the transition for moving to the given slide index.
    ///
    /// Uses per-slide override if set, otherwise the default transition.
    pub fn transition_for(&self, target_index: usize) -> &SlideTransition {
        self.slide_transitions
            .get(&target_index)
            .unwrap_or(&self.default_transition)
    }

    /// Set a per-slide transition override.
    pub fn set_transition(&mut self, slide_index: usize, transition: SlideTransition) {
        self.slide_transitions.insert(slide_index, transition);
    }

    /// Remove a per-slide transition override (revert to default).
    pub fn clear_transition(&mut self, slide_index: usize) {
        self.slide_transitions.remove(&slide_index);
    }

    /// Reorder a slide from one position to another.
    ///
    /// Returns `true` if the reorder was valid, `false` if indices were out of range.
    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        if from >= self.slide_ids.len() || to >= self.slide_ids.len() {
            return false;
        }
        let id = self.slide_ids.remove(from);
        self.slide_ids.insert(to, id);

        // Adjust current index to follow the currently-viewed slide
        if self.current_index == from {
            self.current_index = to;
        } else if from < self.current_index && to >= self.current_index {
            self.current_index = self.current_index.saturating_sub(1);
        } else if from > self.current_index && to <= self.current_index {
            self.current_index = (self.current_index + 1).min(self.slide_ids.len() - 1);
        }

        true
    }

    /// Insert a new slide at the given position.
    pub fn insert_slide(&mut self, index: usize, slide_id: Uuid) {
        let clamped = index.min(self.slide_ids.len());
        self.slide_ids.insert(clamped, slide_id);
        // Adjust current index if inserting before it
        if clamped <= self.current_index && !self.slide_ids.is_empty() {
            self.current_index = (self.current_index + 1).min(self.slide_ids.len() - 1);
        }
    }

    /// Remove a slide by ID. Returns `true` if found and removed.
    pub fn remove_slide(&mut self, slide_id: Uuid) -> bool {
        if let Some(pos) = self.slide_ids.iter().position(|&id| id == slide_id) {
            self.slide_ids.remove(pos);
            // Adjust current index
            if self.current_index >= self.slide_ids.len() && !self.slide_ids.is_empty() {
                self.current_index = self.slide_ids.len() - 1;
            }
            if self.slide_ids.is_empty() {
                self.current_index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all slide IDs for thumbnail strip navigation.
    pub fn thumbnail_strip(&self) -> &[Uuid] {
        &self.slide_ids
    }

    /// Toggle presenter view on/off.
    pub fn toggle_presenter_view(&mut self) {
        self.presenter.enabled = !self.presenter.enabled;
    }

    /// Update the elapsed timer (called by the platform layer each second).
    pub fn tick(&mut self, delta_secs: f64) {
        self.presenter.elapsed_secs += delta_secs;
    }

    /// Reset the elapsed timer.
    pub fn reset_timer(&mut self) {
        self.presenter.elapsed_secs = 0.0;
    }
}

impl Default for SlideSequenceState {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ids(n: usize) -> Vec<Uuid> {
        (0..n).map(|_| Uuid::new_v4()).collect()
    }

    #[test]
    fn new_sequence_starts_at_zero() {
        let ids = make_ids(3);
        let seq = SlideSequenceState::new(ids.clone());
        assert_eq!(seq.current_index, 0);
        assert_eq!(seq.slide_count(), 3);
        assert_eq!(seq.current_slide_id(), Some(ids[0]));
    }

    #[test]
    fn empty_sequence() {
        let seq = SlideSequenceState::new(vec![]);
        assert_eq!(seq.slide_count(), 0);
        assert!(seq.current_slide_id().is_none());
        assert!(seq.is_first());
        assert!(seq.is_last());
    }

    #[test]
    fn navigation_next_previous() {
        let ids = make_ids(3);
        let mut seq = SlideSequenceState::new(ids.clone());

        assert!(seq.is_first());
        assert!(!seq.is_last());

        assert!(seq.next());
        assert_eq!(seq.current_index, 1);
        assert_eq!(seq.current_slide_id(), Some(ids[1]));

        assert!(seq.next());
        assert_eq!(seq.current_index, 2);
        assert!(seq.is_last());

        assert!(!seq.next()); // can't go past end
        assert_eq!(seq.current_index, 2);

        assert!(seq.previous());
        assert_eq!(seq.current_index, 1);

        assert!(seq.previous());
        assert_eq!(seq.current_index, 0);
        assert!(seq.is_first());

        assert!(!seq.previous()); // can't go before start
    }

    #[test]
    fn go_to_valid_index() {
        let ids = make_ids(5);
        let mut seq = SlideSequenceState::new(ids.clone());
        assert!(seq.go_to(3));
        assert_eq!(seq.current_index, 3);
        assert_eq!(seq.current_slide_id(), Some(ids[3]));
    }

    #[test]
    fn go_to_invalid_index() {
        let mut seq = SlideSequenceState::new(make_ids(3));
        assert!(!seq.go_to(5));
        assert_eq!(seq.current_index, 0);
    }

    #[test]
    fn next_slide_preview() {
        let ids = make_ids(3);
        let mut seq = SlideSequenceState::new(ids.clone());
        assert_eq!(seq.next_slide_id(), Some(ids[1]));

        seq.go_to(2);
        assert!(seq.next_slide_id().is_none()); // last slide, no next
    }

    #[test]
    fn previous_slide() {
        let ids = make_ids(3);
        let mut seq = SlideSequenceState::new(ids.clone());
        assert!(seq.previous_slide_id().is_none()); // first slide, no previous

        seq.go_to(1);
        assert_eq!(seq.previous_slide_id(), Some(ids[0]));
    }

    #[test]
    fn per_slide_transition() {
        let mut seq = SlideSequenceState::new(make_ids(3));
        let custom = SlideTransition {
            effect: TransitionEffect::Push,
            duration_secs: 1.0,
            direction: TransitionDirection::Right,
        };
        seq.set_transition(1, custom.clone());

        // Slide 0 uses default
        assert_eq!(seq.transition_for(0).effect, TransitionEffect::Fade);
        // Slide 1 uses override
        assert_eq!(seq.transition_for(1).effect, TransitionEffect::Push);
        assert_eq!(seq.transition_for(1).direction, TransitionDirection::Right);

        seq.clear_transition(1);
        assert_eq!(seq.transition_for(1).effect, TransitionEffect::Fade);
    }

    #[test]
    fn reorder_slides() {
        let ids = make_ids(4);
        let mut seq = SlideSequenceState::new(ids.clone());
        seq.go_to(0);

        // Move slide 0 to position 2
        assert!(seq.reorder(0, 2));
        assert_eq!(seq.slide_ids[0], ids[1]);
        assert_eq!(seq.slide_ids[1], ids[2]);
        assert_eq!(seq.slide_ids[2], ids[0]);
        assert_eq!(seq.slide_ids[3], ids[3]);
    }

    #[test]
    fn reorder_invalid_indices() {
        let mut seq = SlideSequenceState::new(make_ids(3));
        assert!(!seq.reorder(0, 5));
        assert!(!seq.reorder(5, 0));
    }

    #[test]
    fn insert_slide() {
        let ids = make_ids(2);
        let mut seq = SlideSequenceState::new(ids.clone());
        let new_id = Uuid::new_v4();
        seq.insert_slide(1, new_id);
        assert_eq!(seq.slide_count(), 3);
        assert_eq!(seq.slide_ids[1], new_id);
    }

    #[test]
    fn remove_slide() {
        let ids = make_ids(3);
        let mut seq = SlideSequenceState::new(ids.clone());
        seq.go_to(2);
        assert!(seq.remove_slide(ids[2]));
        assert_eq!(seq.slide_count(), 2);
        assert_eq!(seq.current_index, 1); // adjusted to last valid
    }

    #[test]
    fn remove_nonexistent_slide() {
        let mut seq = SlideSequenceState::new(make_ids(2));
        assert!(!seq.remove_slide(Uuid::new_v4()));
    }

    #[test]
    fn thumbnail_strip() {
        let ids = make_ids(5);
        let seq = SlideSequenceState::new(ids.clone());
        assert_eq!(seq.thumbnail_strip(), &ids);
    }

    #[test]
    fn presenter_view_toggle() {
        let mut seq = SlideSequenceState::new(make_ids(1));
        assert!(!seq.presenter.enabled);
        seq.toggle_presenter_view();
        assert!(seq.presenter.enabled);
        seq.toggle_presenter_view();
        assert!(!seq.presenter.enabled);
    }

    #[test]
    fn timer_operations() {
        let mut seq = SlideSequenceState::new(make_ids(1));
        seq.tick(1.5);
        seq.tick(2.0);
        assert!((seq.presenter.elapsed_secs - 3.5).abs() < f64::EPSILON);

        seq.reset_timer();
        assert_eq!(seq.presenter.elapsed_secs, 0.0);
    }

    #[test]
    fn serde_roundtrip() {
        let ids = make_ids(3);
        let mut seq = SlideSequenceState::new(ids);
        seq.go_to(1);
        seq.set_transition(2, SlideTransition {
            effect: TransitionEffect::Custom("wipe".into()),
            duration_secs: 0.8,
            direction: TransitionDirection::Up,
        });

        let json = serde_json::to_string(&seq).unwrap();
        let decoded: SlideSequenceState = serde_json::from_str(&json).unwrap();
        assert_eq!(seq, decoded);
    }

    #[test]
    fn transition_effect_serde() {
        let effects = vec![
            TransitionEffect::Fade,
            TransitionEffect::Slide,
            TransitionEffect::Push,
            TransitionEffect::Dissolve,
            TransitionEffect::None,
            TransitionEffect::Custom("zoom".into()),
        ];
        for effect in effects {
            let json = serde_json::to_string(&effect).unwrap();
            let decoded: TransitionEffect = serde_json::from_str(&json).unwrap();
            assert_eq!(effect, decoded);
        }
    }

    #[test]
    fn transition_direction_serde() {
        for dir in [
            TransitionDirection::Left,
            TransitionDirection::Right,
            TransitionDirection::Up,
            TransitionDirection::Down,
        ] {
            let json = serde_json::to_string(&dir).unwrap();
            let decoded: TransitionDirection = serde_json::from_str(&json).unwrap();
            assert_eq!(dir, decoded);
        }
    }

    #[test]
    fn presenter_view_defaults() {
        let pv = PresenterView::default();
        assert!(!pv.enabled);
        assert!(pv.show_next_slide);
        assert!(pv.show_notes);
        assert!(pv.show_timer);
        assert_eq!(pv.elapsed_secs, 0.0);
    }

    #[test]
    fn default_transition() {
        let t = SlideTransition::default();
        assert_eq!(t.effect, TransitionEffect::Fade);
        assert_eq!(t.duration_secs, 0.5);
        assert_eq!(t.direction, TransitionDirection::Left);
    }
}
