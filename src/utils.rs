#[macro_export]
macro_rules! dbg_println {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        println!($($arg)*);
    };
}






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

pub fn select_icon<T: Clone>(min: f64, max: f64, value: f64, icons: &[T]) -> Option<T> {
    if icons.is_empty() || min >= max {
        return None;
    }

    let value = value.clamp(min, max);
    let range = (max - min) as f64;
    let norm = (value - min) as f64 / range;

    let idx = ((norm * icons.len() as f64).floor() as usize).min(icons.len() - 1);

    Some(icons[idx].clone())
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

use cairo::Context;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimationKey {
    NotificationHeight,
    IconsHeight,
    WobHeightRatio
}

pub struct Animation {
    pub id: AnimationKey,
    pub start: f64,
    pub end: f64,
    pub start_time: Instant,
    pub duration: Duration,
}

pub enum Easing {
    #[allow(unused)]
    Linear,
    #[allow(unused)]
    Smooth,
    #[allow(unused)]
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
    pub(crate) icons_ratio: f64,
    pub(crate) wob_height: f64
}

impl FrameModel {
    pub fn new () -> Self {
        FrameModel {
            notif_height_ratio: 0.0,
            icons_ratio: 0.0,
            wob_height: 0.0
        }
    }

    pub fn set(&mut self, id: AnimationKey, val: f64) {
        match id {
            AnimationKey::NotificationHeight => self.notif_height_ratio = val,
            AnimationKey::IconsHeight => self.icons_ratio = val,
            AnimationKey::WobHeightRatio => self.wob_height = val
        }
    }

    pub fn get(&self, id: AnimationKey) -> f64 {
        match id {
            AnimationKey::NotificationHeight => self.notif_height_ratio,
            AnimationKey::IconsHeight => self.icons_ratio,
            AnimationKey::WobHeightRatio => self.wob_height
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

pub fn cr_text_aligned (cr: Context, text: String, x: f64, y: f64, dx: f64, dy: f64) -> (f64, f64) {
    // if v != 0.0 || h != 0.0 {
        let mut x1 = x;
        let mut y1 = y;
        let extents = cr.text_extents(&text).unwrap();
        x1 -= extents.width() * dx;
        y1 -= extents.height() * dy + extents.y_bearing();
        cr.move_to(x1, y1);
        // dbg_println!("({},{}) -> ({},{})   {:?}", &x, &y, &x1, &y1, extents);
    // }
    cr.show_text(&text).ok();
    (extents.width(), extents.height())
}

pub fn rounded_big_hole (cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64, r2: f64, reserved_w: f64, reserved_h: f64, wob_h: f64) {
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -90f64.to_radians(), 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, 90f64.to_radians());

    if wob_h > 0.0 {
        let r2_safe = if wob_h > r2 { r2 } else { wob_h/2.0 };
        let wob_half_width = 100.0;
        cr.arc(x + w/2.0 + r2_safe + wob_half_width, y + h - r2_safe, r2_safe, 90f64.to_radians(), 180f64.to_radians());
        cr.arc_negative(x + w/2.0 - r2_safe + wob_half_width, y + h + r2_safe - wob_h, r2_safe, 0f64.to_radians(), 270f64.to_radians());
        cr.arc_negative(x + w/2.0 + r2_safe - wob_half_width, y + h + r2_safe - wob_h, r2_safe, 270f64.to_radians(), 180f64.to_radians());
        cr.arc(x + w/2.0 - r2_safe - wob_half_width, y + h - r2_safe, r2_safe, 0f64.to_radians(), 90f64.to_radians());
    }
    
    if reserved_h > 0.0 {
        let r2_safe = if reserved_h > r2 { r2 } else { reserved_h/2.0 };
        // dbg_println!("reserved_h: {}", reserved_h);
        cr.arc(x + r2_safe + reserved_w, y + h - r2_safe, r2_safe, 90f64.to_radians(), 180f64.to_radians());
        cr.arc_negative(x - r2_safe + reserved_w, y + h + r2_safe - reserved_h, r2_safe, 0f64.to_radians(), 270f64.to_radians());
        cr.arc(x + r2, y + h - r2 - reserved_h, r2, 90f64.to_radians(), 180f64.to_radians());
    } else {
        cr.arc(x + r, y + h - r, r, 90f64.to_radians(), 180f64.to_radians());
    }
    
    cr.arc(x + r, y + r, r, 180f64.to_radians(), 270f64.to_radians());
    cr.close_path();
}

pub fn rounded_rect (cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -90f64.to_radians(), 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, 90f64.to_radians());

    cr.arc(x + r, y + h - r, r, 90f64.to_radians(), 180f64.to_radians());
    
    cr.arc(x + r, y + r, r, 180f64.to_radians(), 270f64.to_radians());
    cr.close_path();
}