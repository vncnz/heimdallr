const DEFAULT_WHITE: bool = false;
pub fn get_color_gradient(value: f64) -> (f64, f64, f64, f64) {
    get_color_gradient_full(0.0, 1.0, value, false)
}
pub fn get_color_gradient_full(min: f64, max: f64, value: f64, reversed: bool) -> (f64, f64, f64, f64) {
    let clamped = value.clamp(min, max);
    let mut ratio = if (max - min).abs() < f64::EPSILON {
        0.5
    } else {
        (clamped - min) / (max - min)
    };

    if !reversed { ratio = 1.0 - ratio; }
    let sat;
    let hue;
    if DEFAULT_WHITE {
        sat = f64::max(1.0 - (ratio * ratio * ratio), 0.0);
        hue = 60.0 * ratio; // 60 -> 0
    } else {
        sat = 1.0;
        hue = 100.0 * ratio; // 100 -> 0
    }
    let (r, g, b) = hsv_to_rgb(hue, sat, 1.0);

    // format!("#{:02X}{:02X}{:02X}", r, g, b)
    ((r as f64) / 255.0, (g as f64) / 255.0, (b as f64) / 255.0, 1.0)
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    let r = ((r1 + m) * 255.0).round() as u8;
    let g = ((g1 + m) * 255.0).round() as u8;
    let b = ((b1 + m) * 255.0).round() as u8;

    (r, g, b)
}

use std::fs::OpenOptions;
use std::io::Write;

pub fn log_to_file(msg: String) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/heimdallr.log")
        .expect("impossibile aprire log file");
    writeln!(file, "[{}] {}", chrono::Local::now().format("%H:%M:%S%.3f"), msg).unwrap();
}









/* ANIMATION SYSTEM */



use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimationKey {
    NotificationHeight,
    IconsHeight
}

pub struct Animation {
    pub id: AnimationKey,
    pub start: f64,
    pub end: f64,
    pub start_time: Instant,
    pub duration: Duration,
}

pub enum Easing {
    Linear,
    Smooth,
    Smoother,
    EaseOutCubic,
}
fn ease(e: Easing, t: f64) -> f64 {
    let x = t.clamp(0.0, 1.0);
    match e {
        Easing::Linear => x,
        Easing::Smooth => x * x * (3.0 - 2.0 * x),
        Easing::Smoother => x * x * x * (x * (x * 6.0 - 15.0) + 10.0),
        Easing::EaseOutCubic => 1.0 - (1.0 - x).powi(3),
    }
}

pub struct Animator {
    animations: Vec<Animation>,
}

impl Animator {
    pub fn new() -> Self {
        Self { animations: Vec::new() }
    }

    /// Da chiamare nel tuo `_animation_step()`
    pub fn step(&mut self, model: &mut FrameModel) -> bool {
        let now = Instant::now();
        let mut changed = false;

        self.animations.retain(|a| {
            let t = now.duration_since(a.start_time);
            if t >= a.duration {
                model.set(a.id, a.end);
                changed = true;
                return false; // animazione finita
            }

            let ratio = t.as_secs_f64() / a.duration.as_secs_f64();
            
            // Linear animatioons are no good for eyes!
            let eased = ease(Easing::EaseOutCubic, ratio);
            
            let value = a.start + (a.end - a.start) * eased;// ratio;
            model.set(a.id, value);
            changed = true;
            true
        });

        changed
    }

    pub fn animate_property(
        &mut self,
        model: &FrameModel,
        id: AnimationKey,
        to: f64,
        duration_ms: u64,
    ) {
        // rimuovi eventuale animazione su quella proprietà
        self.animations.retain(|a| a.id != id);

        let start = model.get(id);

        self.animations.push(Animation {
            id,
            start,
            end: to,
            start_time: Instant::now(),
            duration: Duration::from_millis(duration_ms),
        });
    }

}

pub struct FrameModel {
    pub(crate) notif_height_ratio: f64,
    pub(crate) icons_ratio: f64
}

impl FrameModel {
    pub fn new () -> Self {
        FrameModel {
            notif_height_ratio: 0.0,
            icons_ratio: 0.0
        }
    }

    pub fn set(&mut self, id: AnimationKey, val: f64) {
        match id {
            AnimationKey::NotificationHeight => self.notif_height_ratio = val,
            AnimationKey::IconsHeight => self.icons_ratio = val
        }
    }

    pub fn get(&self, id: AnimationKey) -> f64 {
        match id {
            AnimationKey::NotificationHeight => self.notif_height_ratio,
            AnimationKey::IconsHeight => self.icons_ratio
        }
    }
}

/*
self.animator.animate_property(
    AnimationKey::NotificationHeight,
    self.alpha,
    1.0,
    Duration::from_millis(120),
    {
        let ptr = &mut self.alpha as *mut f32;
        move |v| unsafe {
            *ptr = v;
        }
    }
);
*/