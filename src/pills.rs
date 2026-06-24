// I'm experimenting with new UI: some of this shit will be spread out in multiple files, ofc!

use cairo::{Context, FontSlant};
use chrono::Local;
use std::time::{Duration, Instant};

use crate::{
    countdown::Countdown,
    dbg_println,
    heimdallr_layer::AlarmIcon,
    security::MicCameraStatus,
    utils::{
        cr_text_aligned, cr_text_layout, cr_text_rotated_mixed, get_color_gradient,
        rounded_rect_gradient, select_icon,
    },
};

pub static PILL_FONT_SIZE: f64 = 14.0;
pub static PILL_MARGIN: f64 = 6.0;

struct AnimationState {
    current_size: (f64, f64),
    target_size: (f64, f64),
    animation_from: (f64, f64),
    animation_start: Option<Instant>,
    animation_duration: Duration,
}

impl AnimationState {
    fn new() -> Self {
        AnimationState {
            current_size: (0.0, 0.0),
            target_size: (0.0, 0.0),
            animation_from: (0.0, 0.0),
            animation_start: None,
            animation_duration: Duration::from_millis(240),
        }
    }

    fn step(&mut self) -> bool {
        if let Some(start) = self.animation_start {
            let elapsed = Instant::now().saturating_duration_since(start);
            let total = self.animation_duration;
            let ratio = (elapsed.as_secs_f64() / total.as_secs_f64()).min(1.0);
            let eased = 1.0 - (1.0 - ratio).powi(3);

            self.current_size = (
                self.animation_from.0 + (self.target_size.0 - self.animation_from.0) * eased,
                self.animation_from.1 + (self.target_size.1 - self.animation_from.1) * eased,
            );

            let still_animating = ratio < 1.0;
            if !still_animating {
                self.current_size = self.target_size;
                self.animation_start = None;
            }

            still_animating
        } else {
            false
        }
    }

    fn set_target(&mut self, new_target: (f64, f64)) -> bool {
        let t = match new_target {
            (0.0, 0.0) => new_target,
            (x, y) => (x + PILL_MARGIN * 2.0, y)
        };
        let changed = self.target_size != t;

        if changed {
            self.animation_from = self.current_size;
            self.target_size = t;
            self.animation_start = Some(Instant::now());
        }

        changed
    }
}

pub trait PillTrait {
    fn draw(&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64);
    fn animation_state(&mut self) -> &mut AnimationState;

    fn step_animation(&mut self) -> bool {
        self.animation_state().step()
    }

    fn get_current_rect(&mut self) -> (f64, f64) {
        self.animation_state().current_size
    }

    fn get_desired_rect(&mut self) -> (f64, f64) {
        self.animation_state().target_size
    }
}

struct PillBase {
    cached_layout: Option<pango::Layout>,
    cached_sizes: Option<(f64, f64)>,
    cached_text: Option<String>,
    cached_color: Option<(f64, f64, f64, f64)>,
}

impl PillBase {
    fn new() -> Self {
        PillBase {
            cached_layout: None,
            cached_sizes: None,
            cached_text: None,
            cached_color: None,
        }
    }

    fn with_size(size: (f64, f64)) -> Self {
        PillBase {
            cached_layout: None,
            cached_sizes: Some(size),
            cached_text: None,
            cached_color: None,
        }
    }

    fn set_layout(
        &mut self,
        layout: pango::Layout,
        sizes: (f64, f64),
        text: String,
        color: (f64, f64, f64, f64),
    ) {
        self.cached_layout = Some(layout);
        self.cached_sizes = Some(sizes);
        self.cached_text = Some(text);
        self.cached_color = Some(color);
    }

    fn clear(&mut self) {
        self.cached_layout = None;
        self.cached_sizes = None;
        self.cached_text = None;
        self.cached_color = None;
    }

    fn draw_centered(&self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64) {
        if let (Some(layout), Some(sizes), Some(color)) =
            (&self.cached_layout, &self.cached_sizes, &self.cached_color)
        {
            cr.set_source_rgba(color.0, color.1, color.2, color.3);
            cr.move_to(
                x + rect_width / 2.0 - sizes.0 / 2.0,
                y + rect_height / 2.0 - sizes.1 / 2.0,
            );
            pangocairo::functions::show_layout(cr, layout);
            dbg_println!("Pill drawn in rect {sizes:?}");
        } else {
            dbg_println!("Pill drawn in rect (0.0, 0.0)");
        }
    }
}

pub struct PillClock {
    base: PillBase,
    animation: AnimationState,
}

impl PillTrait for PillClock {
    fn draw(&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64) {
        self.base.draw_centered(cr, rect_width, rect_height, x, y);
    }

    fn animation_state(&mut self) -> &mut AnimationState {
        &mut self.animation
    }
}

impl PillClock {
    pub fn new() -> Self {
        PillClock {
            base: PillBase::with_size((45.0, 20.0)),
            animation: AnimationState::new(),
        }
    }

    pub fn update_data(&mut self, cr: &cairo::Context) -> bool {
        let date = Local::now();
        let text = date.format("%H:%M").to_string();

        if self.base.cached_text.as_ref() == Some(&text) {
            return false;
        }

        let (layout, sizes) = cr_text_layout(&cr, &text, PILL_FONT_SIZE).unwrap();
        let color = (1.0, 1.0, 1.0, 1.0);

        self.base.set_layout(layout, sizes, text, color);
        self.animation.set_target(sizes);
        true
    }
}

pub struct PillCountdown {
    base: PillBase,
    animation: AnimationState,
    last_status: (bool, String),
}

impl PillTrait for PillCountdown {
    fn draw(&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64) {
        self.base.draw_centered(cr, rect_width, rect_height, x, y);
    }

    fn animation_state(&mut self) -> &mut AnimationState {
        &mut self.animation
    }
}

impl PillCountdown {
    pub fn new() -> Self {
        PillCountdown {
            base: PillBase::with_size((58.0, 20.0)),
            animation: AnimationState::new(),
            last_status: (false, "".into()),
        }
    }

    pub fn update_data(&mut self, cr: &cairo::Context, countdown: Countdown) -> bool {
        let (status, time) = countdown.format_custom_duration();
        if self.last_status.0 == status && self.last_status.1 == time {
            return false;
        }

        self.last_status = (status, time.clone());
        let w = if status { 1.0 } else { countdown.get_warning() };
        let icon = if status { "󱫌" } else { "󱫡" };
        let color = get_color_gradient(w);
        let text = format!("{icon} {time}");

        let (layout, sizes) = cr_text_layout(&cr, &text, PILL_FONT_SIZE).unwrap();
        let target = (sizes.0.max(58.0), sizes.1);

        self.base.set_layout(layout, target, text, color);
        self.animation.set_target(target);
        true
    }
}

pub struct PillBattery {
    base: PillBase,
    animation: AnimationState,
    battery: Option<crate::battery::BatteryStats>,
}

impl PillTrait for PillBattery {
    fn draw(&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64) {
        self.base.draw_centered(cr, rect_width, rect_height, x, y);
    }

    fn animation_state(&mut self) -> &mut AnimationState {
        &mut self.animation
    }
}

impl PillBattery {
    pub fn new() -> Self {
        PillBattery {
            base: PillBase::new(),
            animation: AnimationState::new(),
            battery: None,
        }
    }

    pub fn update_data(&mut self, cr: &cairo::Context, battery: Option<crate::battery::BatteryStats>) -> bool {
        self.battery = battery;

        let target = if let Some(bat) = &self.battery {
            if bat.state == crate::battery::BatteryState::FullyCharged {
                self.base.clear();
                (0.0, 0.0)
            } else {
                let total_mins = bat.eta_minutes.unwrap_or_default().ceil() as u64;
                let hours = total_mins / 60;
                let minutes = total_mins % 60;

                let eta = match (hours, minutes) {
                    (0, 0) => "0s".to_string(),
                    (0, m) => format!("{}m", m),
                    (h, m) => format!("{}h{}m", h, m),
                };

                let bat_symb: String = match bat.state {
                    crate::battery::BatteryState::Charging => format!("󱐋 {}", eta),
                    crate::battery::BatteryState::Discharging => format!("󰯆 {}", eta),
                    crate::battery::BatteryState::NotCharging => "󱞝".into(),
                    _ => {
                        let slice: &[&str] = &[
                            "󰂎", "󰁺", "󰁻", "󰁼", "󰁽", "󰁾", "󰁿", "󰂀", "󰂁", "󰂂", "󰁹",
                        ];
                        select_icon(0.0, 100.0, bat.percentage, slice)
                            .unwrap()
                            .into()
                    }
                };

                let bat_color = match bat.state {
                    crate::battery::BatteryState::Charging => (0.1, 1.0, 0.2, 1.0),
                    crate::battery::BatteryState::Discharging => {
                        get_color_gradient(((100.0 - bat.percentage) / 200.0) + 0.5)
                    }
                    crate::battery::BatteryState::NotCharging => (0.6, 0.6, 1.0, 1.0),
                    crate::battery::BatteryState::FullyCharged => (0.5, 0.5, 0.8, 0.8),
                    _ => (1.0, 1.0, 1.0, 0.4),
                };

                let (layout, sizes) = cr_text_layout(&cr, &bat_symb, PILL_FONT_SIZE).unwrap();
                self.base.set_layout(layout, sizes, bat_symb, bat_color);
                sizes
            }
        } else {
            self.base.clear();
            (0.0, 0.0)
        };

        self.animation.set_target(target);
        true
    }
}

pub struct PillWarnings {
    icons: Vec<AlarmIcon>,
    animation: AnimationState,
}

impl PillTrait for PillWarnings {
    fn draw(&mut self, cr: &Context, _rect_width: f64, rect_height: f64, x: f64, y: f64) {
        let mut x = x;

        cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(16.0);

        for icon in &self.icons {
            if icon.symbol == "󱫡" || icon.symbol == "󱫌" {
                continue;
            }
            cr.set_source_rgba(icon.color.0, icon.color.1, icon.color.2, icon.color.3);
            cr.move_to(x, y + rect_height / 2.0 + 4.0);
            cr.show_text(&icon.symbol).unwrap();
            x += 20.0;
        }
        dbg_println!("PillWarnings drawn in x {x:?}");
    }

    fn animation_state(&mut self) -> &mut AnimationState {
        &mut self.animation
    }

    fn get_current_rect(&mut self) -> (f64, f64) {
        (self.icons.len() as f64 * 20.0, 20.0)
    }

    fn get_desired_rect(&mut self) -> (f64, f64) {
        (self.icons.len() as f64 * 20.0, 20.0)
    }
}

impl PillWarnings {
    pub fn new() -> Self {
        PillWarnings {
            icons: Vec::new(),
            animation: AnimationState::new(),
        }
    }

    pub fn update_data(&mut self, _cr: &cairo::Context, icons: Vec<AlarmIcon>) -> bool {
        self.icons = icons;
        true
    }
}

pub struct PillSecurity {
    base: PillBase,
    animation: AnimationState,
}

impl PillTrait for PillSecurity {
    fn draw(&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64) {
        if self.base.cached_layout.is_some() {
            let r = 2.0;
            rounded_rect_gradient(&cr, x + PILL_MARGIN / 2.0, y + 3.0, rect_width - PILL_MARGIN, rect_height - 6.0, r, vec![(0.0, (1.0, 0.58, 0.0, 1.0))], crate::utils::GradientDirection::Horizontal, false, None);
        }

        self.base.draw_centered(&cr, rect_width, rect_height, x, y);
    }

    fn animation_state(&mut self) -> &mut AnimationState {
        &mut self.animation
    }
}

impl PillSecurity {
    pub fn new() -> Self {
        PillSecurity {
            base: PillBase::new(),
            animation: AnimationState::new()
        }
    }

    pub fn update_data(
        &mut self,
        cr: &cairo::Context,
        security: &MicCameraStatus
    ) -> bool {
        let text = security.mic_active.clone().into_iter().map(|s| format!("󰍬 {s}"))
            .chain(security.camera_active.clone().into_iter().map(|s| format!("󰖠 {s}")))
            .collect::<Vec<_>>().join("  ·  ");

        let target = if text.is_empty() {
            self.base.clear();
            (0.0, 0.0)
        } else {
            let (layout, sizes) = cr_text_layout(&cr, &text, PILL_FONT_SIZE).unwrap();
            let target = (sizes.0, sizes.1);
            self.base
                .set_layout(layout, target, text, (0.0, 0.0, 0.0, 1.0));
            target
        };

        self.animation.set_target(target);
        true
    }

    pub fn need_fullscreen(&self) -> bool {
        false
    }
}