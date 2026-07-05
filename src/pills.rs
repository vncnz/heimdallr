// I'm experimenting with new UI: some of this shit will be spread out in multiple files, ofc!

use cairo::{Context, FontSlant, Format, ImageSurface};
use chrono::Local;
use std::{collections::HashMap, time::{Duration, Instant}};
use colored::Colorize;

use crate::{
    countdown::Countdown, data::{BatteryDevice, UPowerDeviceKind}, dbg_println, heimdallr_layer::AlarmIcon, notifications, security::MicCameraStatus, utils::{cr_text_layout, ease, get_color_gradient, rounded_rect_gradient, select_icon}
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
            animation_duration: Duration::from_millis(500),
        }
    }

    fn with_size(size: (f64, f64)) -> Self {
        AnimationState {
            current_size: size,
            target_size: size,
            animation_from: size,
            animation_start: None,
            animation_duration: Duration::from_millis(500),
        }
    }

    fn step(&mut self) -> bool {
        if let Some(start) = self.animation_start {
            let elapsed = Instant::now().saturating_duration_since(start);
            let total = self.animation_duration;
            let ratio = (elapsed.as_secs_f64() / total.as_secs_f64()).min(1.0);
            // let eased = 1.0 - (1.0 - ratio).powi(3);
            let eased = ease(crate::utils::Easing::Spring, ratio);

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
            (0.0, _y) => new_target,
            (x, y) => (x + PILL_MARGIN * 2.0, y)
        };
        let changed = self.target_size != t;

        let current_size = self.current_size;
        let ct = self.target_size;
        if changed {
            dbg_println!("{} new_target:{t:?} target_size:{ct:?} current_size:{current_size:?}", "changed target!".cyan());
        } else {
            // dbg_println!("{} new_target:{t:?} target_size:{ct:?} current_size:{current_size:?}", "NOT changed target!".blue());
        }

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
            // dbg_println!("Pill drawn in rect {sizes:?}");
        } else {
            // dbg_println!("Pill drawn in rect (0.0, 0.0)");
        }
    }
}

pub struct PillClock {
    base: PillBase,
    animation: AnimationState,
}

impl PillTrait for PillClock {
    fn draw(&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64) {
        // rounded_rect_gradient(&cr, x, y, rect_width, rect_height, 0.0, vec![(0.0, (1.0, 0.0, 0.0, 0.5))], crate::utils::GradientDirection::Horizontal, false, None);
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
            animation: AnimationState::with_size((45.0, 20.0)),
        }
    }

    pub fn update_data(&mut self, cr: &cairo::Context) -> bool {
        let date = Local::now();
        let text = date.format("%H:%M").to_string();

        if self.base.cached_text.as_ref() == Some(&text) {
            return false;
        }

        let (layout, sizes) = cr_text_layout(&cr, &text, PILL_FONT_SIZE, None).unwrap();
        let color = (1.0, 1.0, 1.0, 1.0);

        self.base.set_layout(layout, sizes, text, color);
        dbg_println!("{} target:{sizes:?}", "PillClock update_data".blue());
        self.animation.set_target(sizes);
        true
    }
}

pub struct PillCountdown {
    base: PillBase,
    animation: AnimationState,
    last_status: (bool, String),
    timer: Countdown
}

impl PillTrait for PillCountdown {
    fn draw(&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64) {

        if self.last_status.0 {
            let r = 2.0;
            rounded_rect_gradient(&cr, x + PILL_MARGIN / 2.0, y + 3.0, rect_width - PILL_MARGIN, rect_height - 6.0, r, vec![(0.0, get_color_gradient(1.0))], crate::utils::GradientDirection::Horizontal, false, None); // Alternative red: (1.0, 0.0, 0.41, 1.0)
        }

        self.base.draw_centered(&cr, rect_width, rect_height, x, y);

        // self.base.draw_centered(cr, rect_width, rect_height, x, y);
    }

    fn animation_state(&mut self) -> &mut AnimationState {
        &mut self.animation
    }
}

impl PillCountdown {
    pub fn new() -> Self {
        PillCountdown {
            base: PillBase::new(), // with_size((58.0, 20.0)),
            animation: AnimationState::new(),
            last_status: (false, "".into()),
            timer: Countdown::new()
        }
    }

    pub fn update_data(&mut self, cr: &cairo::Context) -> bool {
        let (status, time) = self.timer.format_custom_duration();
        if self.last_status.0 == status && self.last_status.1 == time {
            return false;
        }

        let target = if self.timer.is_active() {
            self.last_status = (status, time.clone());

            let w = if status { 1.0 } else { self.timer.get_warning() };
            let icon = if status { "󱫌" } else { "󱫡" };
            let color = if status { (0.0, 0.0, 0.0, 1.0) } else { get_color_gradient(w) };
            let text: &str = if self.timer.is_active() { &format!("{icon} {time}") } else { "" };

            let (layout, sizes) = cr_text_layout(&cr, &text, PILL_FONT_SIZE, None).unwrap();
            let target = if self.timer.is_active() { (sizes.0, sizes.1) } else { (0.0, 0.0) };

            self.base.set_layout(layout, target, text.to_string(), color);
            // dbg_println!("{} {target:?}", "countdown target".blue());
            target
        } else {
            self.base.clear();
            // dbg_println!("{} zero", "countdown target".blue());
            (0.0, 0.0)
        };

        self.animation.set_target(target)
    }
}

pub struct PillLaptopBattery {
    base: PillBase,
    animation: AnimationState,
    battery: Option<crate::battery::BatteryStats>,
}

impl PillTrait for PillLaptopBattery {
    fn draw(&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64) {
        self.base.draw_centered(cr, rect_width, rect_height, x, y);
    }

    fn animation_state(&mut self) -> &mut AnimationState {
        &mut self.animation
    }
}

impl PillLaptopBattery {
    pub fn new() -> Self {
        PillLaptopBattery {
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
                        ].as_slice();
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

                let (layout, sizes) = cr_text_layout(&cr, &bat_symb, PILL_FONT_SIZE, None).unwrap();
                self.base.set_layout(layout, sizes, bat_symb, bat_color);
                sizes
            }
        } else {
            self.base.clear();
            (0.0, 0.0)
        };

        // dbg_println!("{}", "PillBattery update_data".blue());
        self.animation.set_target(target);
        true
    }
}

pub struct PillWarnings {
    icons: Vec<AlarmIcon>,
    bases: Vec<PillBase>,
    animation: AnimationState,
}

impl PillTrait for PillWarnings {
    fn draw(&mut self, cr: &Context, _rect_width: f64, rect_height: f64, x: f64, y: f64) {
        // rounded_rect_gradient(&cr, x, y, _rect_width, rect_height, 0.0, vec![(0.0, (1.0, 0.0, 0.0, 0.35))], crate::utils::GradientDirection::Horizontal, false, None);

        let mut x = x + PILL_MARGIN;

        for b in &self.bases {
            let width = b.cached_sizes.unwrap_or_default().0;
            b.draw_centered(cr, width, rect_height, x, y);
            x += width + 4.0;
        }
    }

    fn animation_state(&mut self) -> &mut AnimationState {
        &mut self.animation
    }

    fn get_current_rect(&mut self) -> (f64, f64) {
        let sizes = self.animation_state().current_size;
        sizes
    }
}

impl PillWarnings {
    pub fn new() -> Self {
        PillWarnings {
            icons: Vec::new(),
            bases: Vec::new(),
            animation: AnimationState::new(),
        }
    }

    pub fn update_data(&mut self, cr: &cairo::Context, icons: Vec<AlarmIcon>) -> bool {
        self.icons = icons;
        // self.bases = self.icons.iter().map(|b| )
        let mut w = 0.0;
        self.bases = Vec::new();
        for i in &self.icons {
            // dbg_println!("{} w:{w} icon:{:?} warn:{:?}", "PillWarnings update_data icon".red(), i.symbol, i.warn);
            let (layout, sizes) = cr_text_layout(&cr, &i.symbol, PILL_FONT_SIZE, None).unwrap();
            let color = get_color_gradient(i.warn);
            let mut base = PillBase::new();
            if w > 0.0 { w += 4.0; }
            w += sizes.0;
            base.set_layout(layout, sizes, i.symbol.clone(), color);
            self.bases.push(base);
        }

        let changed = w != self.animation.target_size.0;
        // dbg_println!("{} w:{w} target:{} current:{} changed:{changed}", "PillWarnings update_data icon".red(), self.animation.target_size.0, self.animation.current_size.0);
        if changed {
            let sizes = (w, 20.0);
            let old = self.animation.target_size;
            // dbg_println!("{} new_target:{sizes:?} old_target:{old:?}", "PillWarnings update_data".blue());
            self.animation.set_target(sizes);
        }
        changed
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
            let (layout, sizes) = cr_text_layout(&cr, &text, PILL_FONT_SIZE, None).unwrap();
            let target = (sizes.0, sizes.1);
            self.base
                .set_layout(layout, target, text, (0.0, 0.0, 0.0, 1.0));
            target
        };

        dbg_println!("{}", "PillSecurity update_data".blue());
        self.animation.set_target(target)
    }
}


pub struct PillDevices {
    batteries: Vec<BatteryDevice>,
    bases: Vec<PillBase>,
    animation: AnimationState,
}

impl PillTrait for PillDevices {
    fn draw(&mut self, cr: &Context, _rect_width: f64, rect_height: f64, x: f64, y: f64) {

        // rounded_rect_gradient(&cr, x, y, _rect_width, rect_height, 0.0, vec![(0.0, (1.0, 0.0, 0.0, 0.5))], crate::utils::GradientDirection::Horizontal, false, None);

        let mut x = x + PILL_MARGIN;

        for b in &self.bases {
            let width = b.cached_sizes.unwrap_or_default().0;
            b.draw_centered(cr, width, rect_height, x, y);
            x += width + 4.0;
        }
    }

    fn animation_state(&mut self) -> &mut AnimationState {
        &mut self.animation
    }

    fn get_current_rect(&mut self) -> (f64, f64) {
        let sizes = self.animation_state().current_size;
        // dbg_println!("{} current_size:{sizes:?}", "PillDevices get_current_rect".red());
        sizes
    }
}

impl PillDevices {
    pub fn new() -> Self {
        PillDevices {
            batteries: Vec::new(),
            bases: Vec::new(),
            animation: AnimationState::new(),
        }
    }

    pub fn update_data(&mut self, cr: &cairo::Context, batteries: Vec<BatteryDevice>) -> bool {
        // let changed_size = self.batteries.len() != batteries.len();
        self.batteries = batteries;
        
        let mut w = 0.0;
        self.bases = Vec::new();
        for b in &self.batteries {
            let icon = match b.kind {
                UPowerDeviceKind::Mouse => if b.is_bluetooth { "󰦋" } else { "󰍽" },
                UPowerDeviceKind::Phone => if b.is_bluetooth { "󰏳" } else { "󰏲" },
                UPowerDeviceKind::Tablet => "",
                UPowerDeviceKind::RemoteControl => "󰻅",
                UPowerDeviceKind::Speakers => "󰦢",
                UPowerDeviceKind::Headphones => "󰥰",
                UPowerDeviceKind::GamingInput => "󱤙",
                UPowerDeviceKind::Keyboard => "󰌌",
                _ => "󰂱"
            };
            let text = format!("{icon} {:.0}%", b.percentage);
            let (layout, sizes) = cr_text_layout(&cr, &text, PILL_FONT_SIZE, None).unwrap();
            let color = get_color_gradient(b.warn);
            let mut base = PillBase::new();
            if w > 0.0 { w += 4.0; }
            w += sizes.0; // layout.width() as f64;
            base.set_layout(layout, sizes, text, color);
            self.bases.push(base);

            /* let sizes = (self.batteries.len() as f64 * (PILL_FONT_SIZE + 2.0), 20.0);
            dbg_println!("{} target:{sizes:?}", "PillBatteries update_data".blue());
            self.animation.set_target(sizes); */
        }
        
        let changed = w != self.animation.target_size.0;
        if changed {
            let sizes = (w, 20.0);
            dbg_println!("{} target:{sizes:?}", "PillDevices update_data".blue());
            self.animation.set_target(sizes);
        }
        changed
    }
}


















pub struct PillNotificationFull {
    appname_base: PillBase,
    body_base: PillBase,
    animation: AnimationState,
    last_notification: Option<crate::notifications::Notification>
}

impl PillTrait for PillNotificationFull {
    fn draw(&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64) {
        // self.body_base.draw_centered(&cr, rect_width, rect_height, x, y);

        let mut x = x + PILL_MARGIN;
        let mut y = y;

        let sizes = self.appname_base.cached_sizes.unwrap_or_default();
        self.appname_base.draw_centered(cr, sizes.0, sizes.1, x, y);
        y += sizes.1 + 4.0;

        let sizes = self.body_base.cached_sizes.unwrap_or_default();
        self.body_base.draw_centered(cr, sizes.0, sizes.1, x, y);
        y += sizes.1 + 4.0;
    }

    fn animation_state(&mut self) -> &mut AnimationState {
        &mut self.animation
    }
}

impl PillNotificationFull {
    pub fn new() -> Self {
        PillNotificationFull {
            appname_base: PillBase::new(),
            body_base: PillBase::new(),
            animation: AnimationState::new(),
            last_notification: None
        }
    }

    pub fn update_data(&mut self, cr: &cairo::Context, notifications: &Vec<crate::notifications::Notification>) -> bool {
        let new_notif = notifications.last().cloned();
        // let changed = self.last_notification != new_notif;
        /* if changed {
            self.last_notification = new_notif;
            // eprintln!("{} last_notification: {:?}", "PillContainer update_data_notifications".red(), self.last_notification);
            /* match self.last_notification {
                None => { self.mode = PillMode::Normal; }
                Some(_) => { self.mode = PillMode::Notification; }
            } */
        } */

        let target = if let Some(notif) = new_notif {

            let white = (1.0, 1.0, 1.0, 1.0);

            let (appname_layout, appname_sizes) = cr_text_layout(&cr, &notif.app_name, PILL_FONT_SIZE - 3.0, Some(500.0)).unwrap();

            let target = (appname_sizes.0, appname_sizes.1);
            
            self.appname_base.set_layout(appname_layout, target, notif.app_name.to_string(), white);
            
            
            // let datetime = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            // let (datetime_layout, datetime_sizes) = cr_text_layout(&cr, &datetime, PILL_FONT_SIZE, Some(500.0)).unwrap();

            
            // let text: &str = "EXAMPLE NOTIFICATION VERY LONG TEXT THAT SHOULD BE BALANCED AND WRAPPED IN THE PILL, BUT IT'S NOT IMPLEMENTED YET. THIS IS JUST A PLACEHOLDER FOR NOW.";
            let text = if notif.body.is_empty() { notif.summary } else { notif.body };

            let (body_layout, body_sizes) = cr_text_layout(&cr, &text, PILL_FONT_SIZE, Some(500.0)).unwrap();
            let target = (body_sizes.0, body_sizes.1);

            self.body_base.set_layout(body_layout, target, text.to_string(), white);
            // dbg_println!("{} {target:?}", "Notification target".blue());
            (
                appname_sizes.0.max(body_sizes.0),
                appname_sizes.1 + body_sizes.1 + 4.0
            )
        } else {
            self.appname_base.clear();
            self.body_base.clear();
            // dbg_println!("{} zero", "Notification target".blue());
            (0.0, 0.0)
        };

        self.animation.set_target(target)
    }

}




















enum PillMode {
    Normal,
    Notification
}

pub struct PillContainer {
    mode: PillMode,
    last_notification: Option<crate::notifications::Notification>,
    animation: AnimationState,
    dummy_surface: cairo::ImageSurface,
    dummy_context: cairo::Context,
    // normal_target: (f64, f64),

    pill_clock: PillClock,
    pill_battery: PillLaptopBattery,
    pill_warnings: PillWarnings,
    pill_security: PillSecurity,
    pill_countdown: PillCountdown,
    pill_devices: PillDevices,

    pill_clock_rect: (f64, f64),
    pill_battery_rect: (f64, f64),
    pill_warnings_rect: (f64, f64),
    pill_security_rect: (f64, f64),
    pill_countdown_rect: (f64, f64),
    pill_devices_rect: (f64, f64),

    pill_notification_full: PillNotificationFull,
    pill_notification_full_rect: (f64, f64)
}

impl PillTrait for PillContainer {
    fn animation_state(&mut self) -> &mut AnimationState {
        &mut self.animation
    }

    fn draw(&mut self, cr: &Context, _rect_width: f64, rect_height: f64, x: f64, y: f64) {
        match self.mode {
            PillMode::Normal => self.draw_normal(cr, _rect_width, rect_height, x, y),
            PillMode::Notification => self.draw_notification(cr, _rect_width, rect_height, x, y)
        }
    }

    fn step_animation(&mut self) -> bool {
        let mut animating = self.animation.step();

        animating |= self.pill_clock.step_animation();
        animating |= self.pill_battery.step_animation();
        animating |= self.pill_warnings.step_animation();
        animating |= self.pill_security.step_animation();
        animating |= self.pill_countdown.step_animation();
        animating |= self.pill_devices.step_animation();
        animating |= self.pill_notification_full.step_animation();

        animating
    }

    fn get_current_rect(&mut self) -> (f64, f64) {
        self.animation.current_size
    }

    fn get_desired_rect(&mut self) -> (f64, f64) {
        self.animation.target_size
    }
}

impl PillContainer {
    pub fn new() -> Self {
        let dummy_surface = ImageSurface::create(Format::ARgb32, 1, 1).unwrap();
        let dummy_context = Context::new(&dummy_surface).unwrap();
        PillContainer {
            mode: PillMode::Normal,
            last_notification: None,
            animation: AnimationState::new(),
            dummy_surface,
            dummy_context,

            pill_clock: PillClock::new(),
            pill_battery: PillLaptopBattery::new(),
            pill_warnings: PillWarnings::new(),
            pill_security: PillSecurity::new(),
            pill_countdown: PillCountdown::new(),
            pill_devices: PillDevices::new(),
            pill_notification_full: PillNotificationFull::new(),    
            pill_clock_rect: (0.0, 0.0),
            pill_battery_rect: (0.0, 0.0),
            pill_warnings_rect: (0.0, 0.0),
            pill_security_rect: (0.0, 0.0),
            pill_countdown_rect: (0.0, 0.0),
            pill_devices_rect: (0.0, 0.0),
            pill_notification_full_rect: (0.0, 0.0)
        }
    }

    pub fn update_data_clock(&mut self) -> bool {
        let changed = self.pill_clock.update_data(&self.dummy_context);
        if changed { self.pill_clock_rect = self.pill_clock.get_current_rect(); }
        return changed
    }

    pub fn set_countdown (&mut self, input: &str) -> Result<u64, &'static str> {
        self.pill_countdown.timer.fill_from_timespan(input) // FIXME: this should be in the pill itself
    }

    pub fn update_data_battery(&mut self, battery: Option<crate::battery::BatteryStats>) -> bool {
        let changed = self.pill_battery.update_data(&self.dummy_context, battery);
        if changed { self.pill_battery_rect = self.pill_battery.get_current_rect(); }
        return changed
    }

    pub fn update_data_warnings(&mut self, icons: &HashMap<String, AlarmIcon>) -> bool {
        let icons: Vec<AlarmIcon> = icons.values().cloned().filter(|icon| icon.symbol != "󱫡" && icon.symbol != "󱫌").collect();
        let changed = self.pill_warnings.update_data(&self.dummy_context, icons);
        if changed { self.pill_warnings_rect = self.pill_warnings.get_current_rect(); }
        return changed
    }

    pub fn update_data_countdown(&mut self) -> bool {
        let changed = self.pill_countdown.update_data(&self.dummy_context);
        if changed { self.pill_countdown_rect = self.pill_countdown.get_current_rect(); }
        return changed
    }

    pub fn update_data_security(&mut self, security: &MicCameraStatus) -> bool {
        let changed = self.pill_security.update_data(&self.dummy_context, security);
        if changed { self.pill_security_rect = self.pill_security.get_current_rect(); }
        return changed
    }

    pub fn update_data_devices(&mut self, batteries: Vec<BatteryDevice>) -> bool {
        let changed = self.pill_devices.update_data(&self.dummy_context, batteries);
        if changed { self.pill_devices_rect = self.pill_devices.get_current_rect(); }
        return changed
    }
    
    pub fn update_data_notifications(&mut self, notifications: &Vec<crate::notifications::Notification>) -> bool {
        let new_notif = notifications.last().cloned();
        let notification_changed = self.pill_notification_full.update_data(&self.dummy_context, notifications);
        let changed = self.last_notification != new_notif || notification_changed;

        if changed {
            self.last_notification = new_notif;

            if self.last_notification.is_some() {
                self.mode = PillMode::Notification;
                self.animation.set_target(self.pill_notification_full.get_desired_rect());
            } else {
                self.mode = PillMode::Normal;
                self.recalculate_normal_target();
            }
        }

        changed
    }

    fn draw_notification(&mut self, cr: &Context, _rect_width: f64, rect_height: f64, x: f64, y: f64) {
        self.pill_notification_full.draw(&cr, _rect_width, rect_height, x, y);
    }

    fn draw_normal(&mut self, cr: &Context, _rect_width: f64, rect_height: f64, x: f64, y: f64) {
        self.sync_child_rects_for_draw();
        // self.recalculate_normal_target();
        /* if self.first_draw {
            self.recalculate_normal_target();
            self.first_draw = false;
        } */

        let mut x = x;
        self.pill_clock.draw(&cr, self.pill_clock_rect.0, rect_height, x, y);
        x += self.pill_clock_rect.0;

        if self.pill_battery_rect.0 > 0.0 {
            self.pill_battery.draw(&cr, self.pill_battery_rect.0, rect_height, x, y);
            x += self.pill_battery_rect.0;
        }

        if self.pill_countdown_rect.0 > 0.0 {
            self.pill_countdown.draw(&cr, self.pill_countdown_rect.0, rect_height, x, y);
            x += self.pill_countdown_rect.0;
        }

        if self.pill_security_rect.0 > 0.0 {
            self.pill_security.draw(&cr, self.pill_security_rect.0, rect_height, x, y);
            x += self.pill_security_rect.0;
        }

        if self.pill_devices_rect.0 > 0.0 {
            self.pill_devices.draw(&cr, self.pill_devices_rect.0, rect_height, x, y);
            x += self.pill_devices_rect.0;
        }

        if self.pill_warnings_rect.0 > 0.0 {
            self.pill_warnings.draw(&cr, self.pill_warnings_rect.0, rect_height, x, y);
            x += self.pill_warnings_rect.0;
        }
        // dbg_println!("PillContainer drawn in x {x:?}");
    }

    pub fn recalculate_normal_target(&mut self) {
        /* self.pill_clock_rect = self.pill_clock.get_desired_rect();
        self.pill_battery_rect = self.pill_battery.get_desired_rect();
        self.pill_warnings_rect = self.pill_warnings.get_desired_rect();
        self.pill_countdown_rect = self.pill_countdown.get_desired_rect();
        self.pill_security_rect = self.pill_security.get_desired_rect();
        self.pill_devices_rect = self.pill_devices.get_desired_rect(); */

        let rect_width =
            self.pill_clock.get_desired_rect().0 +
            if self.pill_battery.get_desired_rect().0 > 0.0 { self.pill_battery.get_desired_rect().0 } else { 0.0 } +
            if self.pill_warnings.get_desired_rect().0 > 0.0 { self.pill_warnings.get_desired_rect().0 } else { 0.0 } +
            if self.pill_countdown.get_desired_rect().0 > 0.0 { self.pill_countdown.get_desired_rect().0 } else { 0.0 } +
            if self.pill_security.get_desired_rect().0 > 0.0 { self.pill_security.get_desired_rect().0 } else { 0.0 } +
            if self.pill_devices.get_desired_rect().0 > 0.0 { self.pill_devices.get_desired_rect().0 } else { 0.0 };
        
        dbg_println!("{} recalculate_normal_target rect_width:{rect_width:?}", "PillContainer".blue());
        dbg_println!("{} recalculate_normal_target rects: clock:{:?} battery:{:?} warnings:{:?} countdown:{:?} security:{:?} devices:{:?}", "PillContainer".blue(), self.pill_clock_rect, self.pill_battery_rect, self.pill_warnings_rect, self.pill_countdown_rect, self.pill_security_rect, self.pill_devices_rect);

        let rect_height = 26.0;
        let fake_width = rect_width - PILL_MARGIN * 2.0; //? Well, it's a bit of a hack, but it works. We reuse the same anomation system of components, but we don't want to add margins in this case. Margins are already added in set_target, so we can just subtract them here.
        // self.animation.set_target((fake_width, rect_height));
        match self.mode {
            PillMode::Normal => {
                // self.normal_target = (fake_width, rect_height);
                self.animation.set_target((fake_width, rect_height));
            }
            PillMode::Notification => {
                // I don't want to change the target when in notification mode, so I cache the value and restore it when switching back to normal mode
                // self.animation.set_target(self.normal_target);
            }
        }
    }

    fn sync_child_rects_for_draw(&mut self) {
        self.pill_clock_rect = self.pill_clock.get_current_rect();
        self.pill_battery_rect = self.pill_battery.get_current_rect();
        self.pill_warnings_rect = self.pill_warnings.get_current_rect();
        self.pill_countdown_rect = self.pill_countdown.get_current_rect();
        self.pill_security_rect = self.pill_security.get_current_rect();
        self.pill_devices_rect = self.pill_devices.get_current_rect();
    }
}