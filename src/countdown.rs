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
        if input.trim().is_empty() || input.trim() == "0" || input.trim() == "off" {
            self.state = None; // Clear the timer if input is empty
            // self.direction = CountdownDirection::Up; // Remove warning
            return Ok(0);
        }

        if input.trim() == "p" {
            // Toggle pause/resume when timer is active
            if self.state.is_some() {
                if self.current_pause_start.is_none() {
                    // start pause
                    self.current_pause_start = Some(Instant::now());
                    return Ok(0);
                } else {
                    return Err("Active timer already paused");
                }
            }
            return Err("No active timer to pause");
        }

        if input.trim() == "r" {
            // Resume when timer is active
            if self.state.is_some() {
                if self.current_pause_start.is_some() {
                    if let Some(pause_start) = self.current_pause_start.take() {
                        self.total_paused_time += pause_start.elapsed();
                        return Ok(0);
                    } else {
                        return Err("Active timer already running");
                    }
                }
            }
            return Err("No active timer to resume");
        }

        if input.trim() == "up" {
            self.state = Some((Instant::now(), Duration::ZERO)); // Start a timer counting up from zero
            // reset any paused state so new timer runs immediately
            self.total_paused_time = Duration::ZERO;
            self.current_pause_start = None;
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
        // set countdown direction for numeric durations
        self.direction = CountdownDirection::Down;
        // reset pause tracking when starting a fresh countdown so it runs immediately
        self.total_paused_time = Duration::ZERO;
        self.current_pause_start = None;

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

        // total paused time including active pause
        let active_pause = self.current_pause_start.map(|t| t.elapsed()).unwrap_or(Duration::ZERO);
        let total_paused = self.total_paused_time + active_pause;

        // effective elapsed time excluding paused durations
        let elapsed_effective = start.elapsed().saturating_sub(total_paused);

        if original_duration.is_zero() {
            return 0.0;
        }

        let ratio = elapsed_effective.as_secs_f64() / original_duration.as_secs_f64();
        if ratio >= 1.0 { 1.0 } else { ratio }
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
        // account for paused time
        let active_pause = self.current_pause_start.map(|t| t.elapsed()).unwrap_or(Duration::ZERO);
        let total_paused = self.total_paused_time + active_pause;
        let elapsed_effective = start.elapsed().saturating_sub(total_paused);

        if elapsed_effective >= total_duration {
            (self.direction == CountdownDirection::Down, elapsed_effective - total_duration)
        } else {
            (false, total_duration.saturating_sub(elapsed_effective))
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

    /* /// Start pausing the countdown. Has no effect if already paused or inactive.
    pub fn pause(&mut self) {
        if self.state.is_some() && self.current_pause_start.is_none() {
            self.current_pause_start = Some(Instant::now());
        }
    }

    /// Resume a paused countdown, adding the paused duration to the accumulator.
    pub fn resume(&mut self) {
        if let Some(pause_start) = self.current_pause_start.take() {
            self.total_paused_time += pause_start.elapsed();
        }
    } */
}
