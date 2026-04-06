use std::collections::VecDeque;

/// Configuration for the jitter buffer.
#[derive(Clone, Debug)]
pub struct JitterConfig {
    /// Minimum buffering delay in milliseconds.
    pub min_delay_ms: u32,
    /// Maximum buffering delay in milliseconds.
    pub max_delay_ms: u32,
    /// Duration of each audio frame in milliseconds (matches Opus frame size).
    pub frame_duration_ms: u32,
}

impl JitterConfig {
    /// Config for voice calls: low latency (20-200ms).
    pub fn voice() -> Self {
        Self {
            min_delay_ms: 20,
            max_delay_ms: 200,
            frame_duration_ms: 20,
        }
    }

    /// Config for music streaming: higher latency tolerance (60-500ms).
    pub fn music() -> Self {
        Self {
            min_delay_ms: 60,
            max_delay_ms: 500,
            frame_duration_ms: 20,
        }
    }
}

impl Default for JitterConfig {
    fn default() -> Self {
        Self::voice()
    }
}

/// A timestamped audio frame.
#[derive(Clone, Debug)]
pub struct TimedFrame {
    /// Sequence number (monotonically increasing per stream).
    pub sequence: u64,
    /// Encoded audio data (e.g. Opus packet).
    pub data: Vec<u8>,
    /// Timestamp in milliseconds (sender's clock).
    pub timestamp_ms: u64,
}

/// Smooths out network timing variations for real-time audio playback.
///
/// Frames are inserted with their sequence numbers and timestamps.
/// The buffer reorders out-of-order packets and waits until enough
/// frames have accumulated before releasing them for playback.
pub struct JitterBuffer {
    buffer: VecDeque<TimedFrame>,
    config: JitterConfig,
    next_sequence: Option<u64>,
    buffering: bool,
    frames_dropped: u64,
    frames_played: u64,
}

impl JitterBuffer {
    pub fn new(config: JitterConfig) -> Self {
        Self {
            buffer: VecDeque::new(),
            config,
            next_sequence: None,
            buffering: true,
            frames_dropped: 0,
            frames_played: 0,
        }
    }

    /// Insert a frame into the buffer. Out-of-order frames are sorted by sequence.
    pub fn push(&mut self, frame: TimedFrame) {
        // Drop frames that are older than what we've already played.
        if let Some(next) = self.next_sequence
            && frame.sequence < next
        {
            self.frames_dropped += 1;
            return;
        }

        // Insert in sorted order by sequence.
        let pos = self
            .buffer
            .iter()
            .position(|f| f.sequence > frame.sequence)
            .unwrap_or(self.buffer.len());

        // Check for duplicate sequence numbers.
        if pos > 0 && self.buffer[pos - 1].sequence == frame.sequence {
            return; // Duplicate.
        }

        self.buffer.insert(pos, frame);

        // Enforce maximum buffer size.
        let max_frames = (self.config.max_delay_ms / self.config.frame_duration_ms) as usize;
        while self.buffer.len() > max_frames {
            self.buffer.pop_front();
            self.frames_dropped += 1;
        }
    }

    /// Get the next frame when ready for playback.
    ///
    /// Returns `None` if still buffering or no frame is available.
    pub fn pop(&mut self) -> Option<TimedFrame> {
        // Initial buffering: wait until we have enough frames.
        if self.buffering {
            let min_frames =
                (self.config.min_delay_ms / self.config.frame_duration_ms).max(1) as usize;
            if self.buffer.len() < min_frames {
                return None;
            }
            self.buffering = false;
        }

        if self.buffer.is_empty() {
            // Buffer underrun — go back to buffering mode.
            self.buffering = true;
            return None;
        }

        let frame = self.buffer.pop_front().expect("buffer verified non-empty above");

        // Track next expected sequence.
        self.next_sequence = Some(frame.sequence + 1);
        self.frames_played += 1;

        Some(frame)
    }

    /// Reset the buffer, clearing all frames and state.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.next_sequence = None;
        self.buffering = true;
    }

    /// Current buffer depth in milliseconds.
    pub fn depth_ms(&self) -> u32 {
        self.buffer.len() as u32 * self.config.frame_duration_ms
    }

    /// Number of frames currently buffered.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Whether the buffer is still in initial buffering mode.
    pub fn is_buffering(&self) -> bool {
        self.buffering
    }

    /// Total frames dropped (late arrivals, overflow).
    pub fn frames_dropped(&self) -> u64 {
        self.frames_dropped
    }

    /// Total frames played.
    pub fn frames_played(&self) -> u64 {
        self.frames_played
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(seq: u64, ts: u64) -> TimedFrame {
        TimedFrame {
            sequence: seq,
            data: vec![seq as u8; 10],
            timestamp_ms: ts,
        }
    }

    #[test]
    fn basic_push_pop() {
        let mut buf = JitterBuffer::new(JitterConfig {
            min_delay_ms: 20,
            max_delay_ms: 200,
            frame_duration_ms: 20,
        });

        // Push one frame — still buffering (need min_delay_ms / frame_duration_ms = 1).
        buf.push(make_frame(0, 0));
        let frame = buf.pop().unwrap();
        assert_eq!(frame.sequence, 0);
    }

    #[test]
    fn buffering_phase() {
        let mut buf = JitterBuffer::new(JitterConfig {
            min_delay_ms: 60,
            max_delay_ms: 200,
            frame_duration_ms: 20,
        });

        // Need 3 frames before releasing (60ms / 20ms = 3).
        buf.push(make_frame(0, 0));
        assert!(buf.pop().is_none());

        buf.push(make_frame(1, 20));
        assert!(buf.pop().is_none());

        buf.push(make_frame(2, 40));
        let frame = buf.pop().unwrap();
        assert_eq!(frame.sequence, 0);
    }

    #[test]
    fn out_of_order_reordering() {
        let mut buf = JitterBuffer::new(JitterConfig {
            min_delay_ms: 40,
            max_delay_ms: 200,
            frame_duration_ms: 20,
        });

        // Arrive out of order: 2, 0, 1.
        buf.push(make_frame(2, 40));
        buf.push(make_frame(0, 0));
        buf.push(make_frame(1, 20));

        // Should come out in order.
        assert_eq!(buf.pop().unwrap().sequence, 0);
        assert_eq!(buf.pop().unwrap().sequence, 1);
        assert_eq!(buf.pop().unwrap().sequence, 2);
    }

    #[test]
    fn duplicate_frames_ignored() {
        let mut buf = JitterBuffer::new(JitterConfig {
            min_delay_ms: 20,
            max_delay_ms: 200,
            frame_duration_ms: 20,
        });

        buf.push(make_frame(0, 0));
        buf.push(make_frame(0, 0)); // duplicate
        buf.push(make_frame(1, 20));

        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn late_frames_dropped() {
        let mut buf = JitterBuffer::new(JitterConfig {
            min_delay_ms: 20,
            max_delay_ms: 200,
            frame_duration_ms: 20,
        });

        buf.push(make_frame(0, 0));
        buf.pop(); // Play frame 0 → next_sequence = 1.

        buf.push(make_frame(1, 20));

        // Late arrival of sequence 0 — should be dropped.
        buf.push(make_frame(0, 0));
        assert_eq!(buf.frames_dropped(), 1);
    }

    #[test]
    fn max_buffer_overflow() {
        let mut buf = JitterBuffer::new(JitterConfig {
            min_delay_ms: 20,
            max_delay_ms: 60,
            frame_duration_ms: 20,
        });

        // Max 3 frames (60ms / 20ms).
        for i in 0..5 {
            buf.push(make_frame(i, i * 20));
        }

        // Should have trimmed to 3 frames, dropping 2 oldest.
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.frames_dropped(), 2);
    }

    #[test]
    fn buffer_underrun_re_enters_buffering() {
        let mut buf = JitterBuffer::new(JitterConfig {
            min_delay_ms: 20,
            max_delay_ms: 200,
            frame_duration_ms: 20,
        });

        buf.push(make_frame(0, 0));
        buf.pop().unwrap();

        // Buffer empty now → underrun.
        assert!(buf.pop().is_none());
        assert!(buf.is_buffering());

        // Need to buffer again before popping.
        buf.push(make_frame(1, 20));
        let frame = buf.pop().unwrap();
        assert_eq!(frame.sequence, 1);
    }

    #[test]
    fn depth_ms() {
        let mut buf = JitterBuffer::new(JitterConfig::voice());
        assert_eq!(buf.depth_ms(), 0);

        buf.push(make_frame(0, 0));
        buf.push(make_frame(1, 20));
        buf.push(make_frame(2, 40));
        assert_eq!(buf.depth_ms(), 60);
    }

    #[test]
    fn reset_clears_state() {
        let mut buf = JitterBuffer::new(JitterConfig::voice());
        buf.push(make_frame(0, 0));
        buf.push(make_frame(1, 20));
        buf.pop();

        buf.reset();
        assert!(buf.is_empty());
        assert!(buf.is_buffering());
        assert_eq!(buf.depth_ms(), 0);
    }

    #[test]
    fn voice_config() {
        let config = JitterConfig::voice();
        assert_eq!(config.min_delay_ms, 20);
        assert_eq!(config.max_delay_ms, 200);
        assert_eq!(config.frame_duration_ms, 20);
    }

    #[test]
    fn music_config() {
        let config = JitterConfig::music();
        assert_eq!(config.min_delay_ms, 60);
        assert_eq!(config.max_delay_ms, 500);
        assert_eq!(config.frame_duration_ms, 20);
    }

    #[test]
    fn stats_tracking() {
        let mut buf = JitterBuffer::new(JitterConfig {
            min_delay_ms: 20,
            max_delay_ms: 200,
            frame_duration_ms: 20,
        });

        buf.push(make_frame(0, 0));
        buf.pop();
        buf.push(make_frame(1, 20));
        buf.pop();

        assert_eq!(buf.frames_played(), 2);
        assert_eq!(buf.frames_dropped(), 0);
    }
}
