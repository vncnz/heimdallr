use std::time::{Duration, Instant};
use regex::Regex;

#[derive(Debug)]
pub struct Countdown {
    pub state: Option<(Instant, Duration)>,
    pub total_paused_time: Duration,
    pub current_pause_start: Option<Instant>,
}

impl Countdown {
    pub fn new () -> Self {
        Countdown {
            state: None, // Some(Instant::now(), Duration.from_millis(1000.0)),
            total_paused_time: Duration::from_millis(0),
            current_pause_start: None
        }
    }

    /// Parses a timespan string like "10m30s" or "45s" and fills the timing property
    pub fn fill_from_timespan(&mut self, input: &str) -> Result<u64, &'static str> {
        // Regex to capture optional minutes and optional seconds
        // e.g., "10m30s", "10m", or "30s"
        let re = Regex::new(r"^(?:(?P<mins>\d+)m)?(?:(?P<secs>\d+)s)?$")
            .map_err(|_| "Failed to compile regex")?;

        let caps = re.captures(input).ok_or("Invalid timespan format")?;

        let mut total_seconds = 0u64;

        // Parse minutes if present
        if let Some(mins_match) = caps.name("mins") {
            let mins: u64 = mins_match.as_str().parse().map_err(|_| "Invalid minutes number")?;
            total_seconds += mins * 60;
        }

        // Parse seconds if present
        if let Some(secs_match) = caps.name("secs") {
            let secs: u64 = secs_match.as_str().parse().map_err(|_| "Invalid seconds number")?;
            total_seconds += secs;
        }

        // Ensure we actually parsed some duration
        if total_seconds == 0 && caps.name("mins").is_none() && caps.name("secs").is_none() {
            return Err("Timespan cannot be empty");
        }

        let duration = Duration::from_secs(total_seconds);
        let now = Instant::now();

        // Fill the property with both the anchor instant and the duration
        self.state = Some((now, duration));

        Ok(total_seconds)
    }

    /// Returns the progress as a float between 0.0 (started) and 1.0 (finished)
    pub fn progress(&self) -> f32 {
        /* let Some((start, total_duration)) = self.state else {
            return 0.0; // Countdown hasn't started
        };

        let elapsed = start.elapsed(); // Shortcut for Instant::now() - start

        if elapsed >= total_duration {
            1.0 // Finished
        } else {
            // Calculate the ratio
            elapsed.as_secs_f32() / total_duration.as_secs_f32()
        } */
        let Some((start, original_duration)) = self.state else { return 0.0; };

        // 1. Calculate active pause time if currently paused
        let active_pause = self.current_pause_start
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO);
        
        // 2. Adjust the total duration forward
        let adjusted_duration = original_duration + self.total_paused_time + active_pause;
        let elapsed = start.elapsed();

        if elapsed >= adjusted_duration {
            1.0
        } else {
            elapsed.as_secs_f32() / adjusted_duration.as_secs_f32()
        }
    }

    /// Returns the remaining formatted time or standard Duration
    pub fn time_remaining(&self) -> Duration {
        let Some((start, total_duration)) = self.state else {
            return Duration::ZERO;
        };

        total_duration.checked_sub(start.elapsed()).unwrap_or(Duration::ZERO)
    }

    pub fn pause(&mut self) {
        if self.current_pause_start.is_none() && self.state.is_some() {
            self.current_pause_start = Some(Instant::now());
        }
    }

    pub fn resume(&mut self) {
        if let Some(pause_start) = self.current_pause_start.take() {
            self.total_paused_time += pause_start.elapsed();
        }
    }
}
