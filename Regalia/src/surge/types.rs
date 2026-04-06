/// Animation curve trait. Maps elapsed time to progress (0.0–1.0, may overshoot for springs).
pub trait Surge: Send + Sync {
    /// Progress at elapsed time t seconds.
    fn value(&self, t: f64) -> f64;

    /// Whether the animation is complete.
    fn is_complete(&self, t: f64, velocity: f64) -> bool {
        let _ = velocity;
        t >= self.duration()
    }

    /// Nominal duration in seconds (0 for instant).
    fn duration(&self) -> f64;
}
