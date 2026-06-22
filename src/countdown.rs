use std::time::{Duration, Instant};
use regex::Regex;

#[derive(Debug, PartialEq, Clone)]
pub enum CountdownDirection {
    Up,
    Down
}

#[derive(Debug)]
pub struct Countdown {
    pub state: Option<(Instant, Duration)>,
    pub total_paused_time: Duration,
    pub current_pause_start: Option<Instant>,
    pub direction: CountdownDirection
}

impl Countdown {
    pub fn new () -> Self {
        Countdown {
            state: None, // Some(Instant::now(), Duration.from_millis(1000.0)),
            total_paused_time: Duration::from_millis(0),
            current_pause_start: None,
            direction: CountdownDirection::Down
        }
    }

    pub fn is_active(&self) -> bool {
        self.state.is_some()
    }

    /// Parses a timespan string like "10m30s" or "45s" and fills the timing property
    pub fn fill_from_timespan(&mut self, input: &str) -> Result<u64, &'static str> {
        self.direction = CountdownDirection::Down;

        if input.trim().is_empty() || input.trim() == "0" || input.trim() == "off" {
            self.state = None; // Clear the timer if input is empty
            return Ok(0);
        }

        if input.trim() == "up" {
            self.state = Some((Instant::now(), Duration::ZERO)); // Start a timer counting up from zero
            self.direction = CountdownDirection::Up;
            return Ok(0);
        }

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
    pub fn progress(&self) -> f64 {
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
            elapsed.as_secs_f64() / adjusted_duration.as_secs_f64()
        }
    }

    pub fn get_warning (&self) -> f64 {
        if self.direction == CountdownDirection::Down {
            self.progress() * 0.5
        } else {
            0.0
        }
    }

    /// Returns the remaining formatted time or standard Duration
    pub fn time_remaining(&self) -> (bool, Duration) {
        let Some((start, total_duration)) = self.state else {
            return (true, Duration::ZERO);
        };

        let dur = start.elapsed();
        if dur >= total_duration {
            (self.direction == CountdownDirection::Down, dur - total_duration)
        } else {
            (false, total_duration - dur) 
        }
    }

    pub fn format_custom_duration(&self) -> (bool, String) {
        let tm = self.time_remaining();
        let (passed, total_secs) = (tm.0, tm.1.as_secs());
        
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;

        match (hours, minutes, seconds) {
            (0, 0, 0) => (passed, "0s".to_string()),
            (0, 0, s) => (passed, format!("{}s", s)),
            (0, m, s) => (passed, format!("{}m{}s", m, s)),
            (h, m, s) => (passed, format!("{}h{}m{}s", h, m, s)),
        }
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
