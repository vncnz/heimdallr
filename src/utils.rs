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
    WobHeightRatio,
    SecurityNotchRatio,
    BatteriesNotchRatio
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
    #[allow(unused)]
    EaseOutCubic,
    #[allow(unused)]
    Spring
}
fn ease(e: Easing, t: f64) -> f64 {
    let x = t.clamp(0.0, 1.0);
    match e {
        Easing::Linear => x,
        Easing::Smooth => x * x * (3.0 - 2.0 * x),
        Easing::Smoother => x * x * x * (x * (x * 6.0 - 15.0) + 10.0),
        Easing::EaseOutCubic => 1.0 - (1.0 - x).powi(3),
        Easing::Spring => {
            if x == 0.0 || x == 1.0 {
                x
            } else {
                /*
                // Adjust factor for frequency (how many bounces) and decay (how fast it settles)
                let factor = 1.01;
                
                // 2.0 ^ (-10 * x) * sin/cos gives the decay
                // This formula ensures it starts at 0 and overshoots/settles beautifully at 1
                1.0 - (2.0f64.powf(-factor * x) * ((x * factor - 0.75) * std::f64::consts::TAU).cos())
                */
                let damping = 12.0;   // Higher = less overshoot (smaller peaks). Lower = higher peaks
                let frequency = 1.15; // Controls the number of bounces

                let decay = 2.0f64.powf(-damping * x);
                let wave = ((x * frequency * std::f64::consts::TAU) - std::f64::consts::FRAC_PI_2).cos();
                
                1.0 - (decay * wave)
            }
        }
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
    pub(crate) wob_height: f64,
    pub(crate) security_height: f64,
    pub(crate) batteries_height: f64
}

impl FrameModel {
    pub fn new () -> Self {
        FrameModel {
            notif_height_ratio: 0.0,
            icons_ratio: 0.0,
            wob_height: 0.0,
            security_height: 0.0,
            batteries_height: 0.0
        }
    }

    pub fn set(&mut self, id: AnimationKey, val: f64) {
        match id {
            AnimationKey::NotificationHeight => self.notif_height_ratio = val,
            AnimationKey::IconsHeight => self.icons_ratio = val,
            AnimationKey::WobHeightRatio => self.wob_height = val,
            AnimationKey::SecurityNotchRatio => self.security_height = val,
            AnimationKey::BatteriesNotchRatio => self.batteries_height = val
        }
    }

    pub fn get(&self, id: AnimationKey) -> f64 {
        match id {
            AnimationKey::NotificationHeight => self.notif_height_ratio,
            AnimationKey::IconsHeight => self.icons_ratio,
            AnimationKey::WobHeightRatio => self.wob_height,
            AnimationKey::SecurityNotchRatio => self.security_height,
            AnimationKey::BatteriesNotchRatio => self.batteries_height
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

use cairo::Error;

pub fn cr_text_rotated(cr: &Context, text: &str, x: f64, y: f64, dx: f64, dy: f64, angle: f64) -> Result<(f64, f64), Error> {
    let extents = cr.text_extents(text)?;

    cr.save()?;

    cr.translate(x, y);

    cr.rotate(angle.to_radians());

    let local_x = -extents.width() * dx;
    let local_y = -(extents.height() * dy + extents.y_bearing());

    /* let layout = pangocairo::functions::create_layout(cr);

    let mut font_desc = pango::FontDescription::new();
    font_desc.set_family(""); // O "Iosevka", o lasci il default
    font_desc.set_absolute_size(10.0 * pango::SCALE as f64);
    layout.set_font_description(Some(&font_desc));
    layout.set_text(text);

    cr.move_to(local_x, local_y);
    pangocairo::functions::show_layout(cr, &layout); */

    cr.move_to(local_x, local_y);
    cr.show_text(text)?;

    cr.restore()?;

    // Restituiamo le dimensioni del testo "dritto", coerentemente con il primo metodo
    Ok((extents.width(), extents.height()))
}

pub fn cr_text_rotated_mixed(cr: &Context, text: &str, x: f64, y: f64, dx: f64, dy: f64, angle: f64, font_size: f64) -> Result<(f64, f64), Error> {

    cr.save()?;

    cr.translate(x, y);

    cr.rotate(angle.to_radians());

    let layout = pangocairo::functions::create_layout(cr);

    let mut font_desc = pango::FontDescription::new();
    font_desc.set_family("");
    font_desc.set_absolute_size(font_size * pango::SCALE as f64);
    layout.set_font_description(Some(&font_desc));
    layout.set_text(text);
    let (_ink_rect, logical_rect) = layout.extents();
    // dbg_println!("ink_rect: {:?}   logical_rect: {:?}", ink_rect, logical_rect);
    let w = logical_rect.width() as f64 / pango::SCALE as f64;
    let h = logical_rect.height() as f64 / pango::SCALE as f64;
    // let extents = cr.text_extents(text)?;
    let local_x = -w * dx;
    let local_y = -(h * dy);

    cr.move_to(local_x, local_y);
    pangocairo::functions::show_layout(cr, &layout);

    /*cr.move_to(local_x, local_y);
    cr.show_text(text)?;*/

    cr.restore()?;

    // Restituiamo le dimensioni del testo "dritto", coerentemente con il primo metodo
    Ok((w, h))
}

pub fn cr_text_layout(cr: &Context, text: &str, font_size: f64) -> Result<(pango::Layout, (f64, f64)), Error> {

    let layout = pangocairo::functions::create_layout(cr);

    let mut font_desc = pango::FontDescription::new();
    font_desc.set_family("");
    font_desc.set_absolute_size(font_size * pango::SCALE as f64);
    layout.set_font_description(Some(&font_desc));
    layout.set_text(text);
    let (_ink_rect, logical_rect) = layout.extents();
    // dbg_println!("ink_rect: {:?}   logical_rect: {:?}", ink_rect, logical_rect);
    let w = logical_rect.width() as f64 / pango::SCALE as f64;
    let h = logical_rect.height() as f64 / pango::SCALE as f64;
    Ok((layout, (w, h)))
}

/* Replace by more general method draw_smart_border
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
} */

/* pub fn rounded_rect (cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -90f64.to_radians(), 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, 90f64.to_radians());

    cr.arc(x + r, y + h - r, r, 90f64.to_radians(), 180f64.to_radians());
    
    cr.arc(x + r, y + r, r, 180f64.to_radians(), 270f64.to_radians());
    cr.close_path();
} */



// use cairo::Context;
use std::f64::consts::PI;

#[derive(PartialEq)]
pub enum Anchor {
    TopLeft, TopCenter, TopRight,
    RightCenter,
    BottomRight, BottomCenter, BottomLeft,
    LeftCenter,
}

pub struct ReservedSpace {
    pub anchor: Anchor,
    pub width: f64,
    pub height: f64,
}

pub enum Side { Top, Bottom, Left, Right }

/// Disegna un notch fluido composto da 4 archi.
/// - `side`: il lato su cui si trova.
/// - `center`: la coordinata lungo il lato (x per Top/Bottom, y per Left/Right).
/// - `depth`: quanto rientra verso l'interno dello schermo.
/// - `width`: la larghezza totale della base del notch.
/// - `r`: il raggio di curvatura (r2 nel tuo codice originale).
fn add_notch(cr: &Context, side: Side, center: f64, edge_pos: f64, width: f64, depth: f64, r: f64) {
    let r = r.min(depth / 2.0).min(width / 4.0);
    let half_w = width / 2.0;

    match side {
        Side::Bottom => {
            // Da destra verso sinistra (seguendo il path orario)
            let x_start = center + half_w;
            let x_end = center - half_w;
            let y_edge = edge_pos;
            let y_deep = edge_pos - depth;

            cr.arc(x_start + r, y_edge - r, r, 0.5 * PI, PI); // Entrata (convessa)
            cr.arc_negative(x_start - r, y_deep + r, r, 0.0, 1.5 * PI); // Interno DX (concava)
            cr.arc_negative(x_end + r, y_deep + r, r, 1.5 * PI, PI); // Interno SX (concava)
            cr.arc(x_end - r, y_edge - r, r, 0.0, 0.5 * PI); // Uscita (convessa)
        }
        Side::Top => {
            // Da sinistra verso destra
            let x_start = center - half_w;
            let x_end = center + half_w;
            let y_edge = edge_pos;
            let y_deep = edge_pos + depth;

            cr.arc(x_start - r, y_edge + r, r, 1.5 * PI, 2.0 * PI);
            cr.arc_negative(x_start + r, y_deep - r, r, PI, 0.5 * PI);
            cr.arc_negative(x_end - r, y_deep - r, r, 0.5 * PI, 0.0);
            cr.arc(x_end + r, y_edge + r, r, PI, 1.5 * PI);
        }
        Side::Right => {
            // Dall'alto verso il basso
            let y_start = center - half_w;
            let y_end = center + half_w;
            let x_edge = edge_pos;
            let x_deep = edge_pos - depth;

            cr.arc(x_edge - r, y_start - r, r, 0.0, 0.5 * PI);
            cr.arc_negative(x_deep + r, y_start + r, r, 1.5 * PI, PI);
            cr.arc_negative(x_deep + r, y_end - r, r, PI, 0.5 * PI);
            cr.arc(x_edge - r, y_end + r, r, 1.5 * PI, 0.0);
        }
        Side::Left => {
            // Dal basso verso l'alto
            let y_start = center + half_w;
            let y_end = center - half_w;
            let x_edge = edge_pos;
            let x_deep = edge_pos + depth;

            cr.arc(x_edge + r, y_start + r, r, PI, 1.5 * PI);
            cr.arc_negative(x_deep - r, y_start - r, r, 0.5 * PI, 0.0);
            cr.arc_negative(x_deep - r, y_end + r, r, 0.0, 1.5 * PI);
            cr.arc(x_edge + r, y_end - r, r, 0.5 * PI, PI);
        }
    }
}

/*pub fn draw_frame(cr: &Context, x: f64, y: f64, w: f64, h: f64, r_base: f64, r_notch: f64, spaces: &[ReservedSpace]) {
    cr.new_sub_path();

    // 1. Angolo Top-Left -> Top-Right
    cr.arc(x + r_base, y + r_base, r_base, PI, 1.5 * PI);
    if let Some(s) = spaces.iter().find(|s| s.anchor == Anchor::TopCenter) {
        add_notch(cr, Side::Top, x + w/2.0, y, s.width, s.height, r_notch);
    }
    cr.arc(x + w - r_base, y + r_base, r_base, 1.5 * PI, 0.0);

    // 2. Lato Right
    if let Some(s) = spaces.iter().find(|s| s.anchor == Anchor::RightCenter) {
        add_notch(cr, Side::Right, y + h/2.0, x + w, s.height, s.width, r_notch);
    }
    cr.arc(x + w - r_base, y + h - r_base, r_base, 0.0, 0.5 * PI);

    // 3. Lato Bottom
    if let Some(s) = spaces.iter().find(|s| s.anchor == Anchor::BottomCenter) {
        add_notch(cr, Side::Bottom, x + w/2.0, y + h, s.width, s.height, r_notch);
    }
    cr.arc(x + r_base, y + h - r_base, r_base, 0.5 * PI, PI);

    // 4. Lato Left
    if let Some(s) = spaces.iter().find(|s| s.anchor == Anchor::LeftCenter) {
        add_notch(cr, Side::Left, y + h/2.0, x, s.height, s.width, r_notch);
    }

    cr.close_path();
}
*/

pub fn draw_smart_border(
    cr: &Context, 
    x: f64, y: f64, w: f64, h: f64, xc: f64, yc: f64,
    r_base: f64, 
    r_notch: f64, 
    spaces: &[ReservedSpace]
) {
    let get_s = |a: Anchor| spaces.iter().find(|s| s.anchor == a);
    cr.new_sub_path();

    if let Some(s) = get_s(Anchor::TopLeft) {
        cr.move_to(x, y + s.height + r_notch);
        cr.arc(x + r_notch, y + s.height - r_notch, r_notch, PI, 1.5 * PI);
        cr.line_to(x + s.width - r_notch, y + s.height - r_notch);
        cr.arc_negative(x + s.width + r_notch, y + s.height + r_notch, r_notch, 1.5 * PI, 0.0);
    } else {
        cr.arc(x + r_base, y + r_base, r_base, PI, 1.5 * PI);
    }

    if let Some(s) = spaces.iter().find(|s| s.anchor == Anchor::TopCenter) {
        add_notch(cr, Side::Top, xc, y, s.width, s.height, r_notch);
        // eprintln!("Notch TopCenter: x {}, w {}, width {}, height {}", x, w, s.width, s.height);
    }

    if let Some(s) = get_s(Anchor::TopRight) {
        let r2_safe = if s.height > r_notch { r_notch } else { s.height/2.0 };
        cr.arc(x + w - s.width - r2_safe, y + r2_safe, r2_safe, 270f64.to_radians(), 0f64.to_radians());
        cr.arc_negative(x + w - s.width + r2_safe, y + s.height - r2_safe, r2_safe, 180f64.to_radians(), 90f64.to_radians());
        cr.arc(x + w - r2_safe, y + s.height + r2_safe, r2_safe, 270f64.to_radians(), 0.0);
    } else {
        cr.arc(x + w - r_base, y + r_base, r_base, 1.5 * PI, 0.0);
    }

    if let Some(s) = spaces.iter().find(|s| s.anchor == Anchor::RightCenter) {
        add_notch(cr, Side::Right, yc, x + w, s.height, s.width, r_notch);
    }

    if let Some(s) = get_s(Anchor::BottomRight) {
        let r2_safe = if s.height > r_notch { r_notch } else { s.height/2.0 };
        cr.arc(x + w - r2_safe, y + h - s.height - r2_safe, r2_safe, 0f64.to_radians(), 90f64.to_radians());
        cr.arc_negative(x + w - s.width + r2_safe, y + h - s.height + r2_safe, r2_safe, 270f64.to_radians(), 180f64.to_radians());
        cr.arc(x + w - r2_safe - s.width, y + h - r2_safe, r2_safe, 0f64.to_radians(), 90f64.to_radians());
    } else {
        cr.arc(x + w - r_base, y + h - r_base, r_base, 0.0, 0.5 * PI);
    }

    if let Some(s) = spaces.iter().find(|s| s.anchor == Anchor::BottomCenter) {
        add_notch(cr, Side::Bottom, xc, y + h, s.width, s.height, r_notch);
    }

   if let Some(s) = get_s(Anchor::BottomLeft) {
        let r2_safe = if s.height > r_notch { r_notch } else { s.height/2.0 };
        // dbg_println!("reserved_h: {}", reserved_h);
        cr.arc(x + s.width + r2_safe, y + h - r2_safe, r2_safe, 90f64.to_radians(), 180f64.to_radians());
        cr.arc_negative(x - r2_safe + s.width, y + h + r2_safe - s.height, r2_safe, 0f64.to_radians(), 270f64.to_radians());
        cr.arc(x + r2_safe, y + h - r2_safe - s.height, r2_safe, 90f64.to_radians(), 180f64.to_radians());
    } else {
        //cr.arc(x + r, y + h - r, r, 90f64.to_radians(), 180f64.to_radians());
        cr.arc(x + r_base, y + h - r_base, r_base, 0.5 * PI, PI);
    }

    if let Some(s) = spaces.iter().find(|s| s.anchor == Anchor::LeftCenter) {
        add_notch(cr, Side::Left, yc, x, s.height, s.width, r_notch);
    }

    cr.close_path();
}












pub enum GradientDirection {
    Vertical,
    Horizontal
}

/// Rounded rect color gradient-colored
/// 
/// # Parameters
/// - cr: Cairo Context
/// - x, y: position
/// - w, h: size
/// - r: border radius
/// - colors: Color couples for gradient, from bottom to top, format (step_point as decimal, [R, G, B, A])
pub fn rounded_rect_gradient(
    cr: &Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    r: f64,
    colors: Vec<(f64, (f64, f64, f64, f64))>,
    direction: GradientDirection,
    use_gradient: bool,
    border_color: Option<(f64,f64,f64,f64)>
) {
    if colors.is_empty() {
        return;
    }

    let gradient = match direction {
        GradientDirection::Vertical => cairo::LinearGradient::new(x, y + h, x, y),
        GradientDirection::Horizontal => cairo::LinearGradient::new(x, y, x + w, y)
    };

    let mut last_color: Option<(f64, f64, f64, f64)> = None;
    for (position, color) in colors {
        if !use_gradient {
            if let Some((r,g,b,a)) = last_color {
                gradient.add_color_stop_rgba(position, r, g, b, a);
            }
        }
        let (red, green, blue, alpha) = color;
        gradient.add_color_stop_rgba(position, red, green, blue, alpha);
        last_color = Some(color);
    }

    cr.set_source(gradient).unwrap();

    // Disegnare il rettangolo con bordi arrotondati
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -90f64.to_radians(), 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, 90f64.to_radians());
    cr.arc(x + r, y + h - r, r, 90f64.to_radians(), 180f64.to_radians());
    cr.arc(x + r, y + r, r, 180f64.to_radians(), 270f64.to_radians());
    cr.close_path();

    // Riempire il rettangolo
    if let Some(c) = border_color {
        cr.fill_preserve().unwrap();

        let (red, green, blue, alpha) = c;
        cr.set_source_rgba(red, green, blue, alpha);
        cr.set_line_width(2.0);
        cr.stroke().unwrap();
    } else {
        cr.fill().unwrap();
    }
}

/* pub fn rounded_rect_no_gradient (
    cr: &Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    r: f64,
    colors: Vec<(f64, [f64; 4])>, // (posizione relativa, colore)
) {
    if colors.is_empty() {
        return;
    }

    // Ordinare i colori per posizione
    let mut sorted_colors = colors;
    sorted_colors.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut prev_y = y + h; // Inizia dal basso
    
    for (i, (position, color)) in sorted_colors.iter().enumerate() {
        let current_y = y + h - (position * h); // Converti posizione relativa in y assoluto
        let block_height = prev_y - current_y;
        
        if block_height > 0.001 {
            let [red, green, blue, alpha] = color;
            cr.set_source_rgba(*red, *green, *blue, *alpha);

            // Disegna solo il primo e l'ultimo con bordi arrotondati
            if i == 0 {
                rounded_rect(cr, x, current_y, w, block_height + 2.0, r);
            } else if i == sorted_colors.len() - 1 {
                rounded_rect(cr, x, current_y - 2.0, w, block_height + 2.0, r);
            } else {
                cr.rectangle(x, current_y, w, block_height);
            }
            cr.fill().unwrap();
        }
        prev_y = current_y;
    }
} */