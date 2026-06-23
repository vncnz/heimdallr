// I'm experimenting with new UI: some of this shit will be spread out in multiple files, ofc!

use cairo::Context;
use chrono::Local;

use crate::{countdown::Countdown, dbg_println, heimdallr_layer::AlarmIcon, utils::{cr_text_aligned, cr_text_layout, cr_text_rotated_mixed, get_color_gradient, select_icon}};
// use enum_dispatch::enum_dispatch;

// use crate::{clock1::Clock1, clock2::Clock2};

pub static PILL_FONT_SIZE: f64 = 14.0;

// #[enum_dispatch]
pub trait PillTrait {
    // fn new () -> Self;
    fn draw (&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64);
    fn get_current_rect (&self) -> (f64, f64);
    fn get_desired_rect (&self) -> (f64, f64);
}

macro_rules! define_pill_trait_implementer {
    ($nome_widget:ident, { $($campo_extra:ident : $tipo_extra:ty),* $(,)? }) => {

        pub struct $nome_widget {
            // Campi comuni a tutti i widget
            cached_layout: Option<pango::Layout>,
            cached_sizes: Option<(f64, f64)>,
            cached_text: Option<String>,
            cached_color: Option<(f64, f64, f64, f64)>,

            // Campi specifici passati alla macro
            $( $campo_extra : $tipo_extra, )*
        }

        impl PillTrait for $nome_widget {
            fn draw (&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64) {

                if let (Some(lay), Some(sizes), Some(_text), Some(color)) = (&self.cached_layout, &self.cached_sizes, &self.cached_text, &self.cached_color) {
                    cr.set_source_rgba(color.0, color.1, color.2, color.3);
                    cr.move_to(x + rect_width / 2.0 - sizes.0 / 2.0, y + rect_height / 2.0 - sizes.1 / 2.0);
                    pangocairo::functions::show_layout(cr, lay);
                    dbg_println!("PillClock drawn in rect {sizes:?}");
                } else {
                    dbg_println!("PillClock drawn in rect (0.0, 0.0)");
                }
            }

            fn get_current_rect (&self) -> (f64, f64) { self.cached_sizes.unwrap_or((0.0, 0.0)) }
            fn get_desired_rect (&self) -> (f64, f64) { self.cached_sizes.unwrap_or((0.0, 0.0)) }
        }
    };
}

define_pill_trait_implementer!(PillClock, {});

impl PillClock {
    pub fn new () -> Self {
        PillClock {
            cached_layout: None,
            cached_sizes: Some((45.0, 20.0)),
            cached_text: None,
            cached_color: Some((1.0, 1.0, 1.0, 1.0))
        }
    }

    pub fn update_data (&mut self, cr: &cairo::Context) {
        let date = Local::now();
        let text = date.format("%H:%M").to_string();

        if self.cached_text.as_ref() == Some(&text) {
            return;
        }

        // cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        let (lay, sizes) = cr_text_layout(&cr, &text, PILL_FONT_SIZE).unwrap();
        self.cached_layout = Some(lay);
        self.cached_sizes = Some(sizes);
        self.cached_text = Some(text);
    }
}





define_pill_trait_implementer!(PillCountdown, {
    countdown: Countdown
});
impl PillCountdown {
    pub fn new (countdown: Countdown) -> Self {
        PillCountdown {
            countdown,
            cached_layout: None,
            cached_sizes: Some((58.0, 20.0)),
            cached_text: None,
            cached_color: Some((1.0, 1.0, 1.0, 1.0))
        }
    }

    pub fn update_data (&mut self, cr: &cairo::Context) {
        let (status, time) = self.countdown.format_custom_duration();
        let w = if status { 1.0 } else { self.countdown.get_warning() };
        let icon = if status { "󱫌" } else { "󱫡" };
        let color = get_color_gradient(w);
        let text = format!("{icon} {time}");

        let (lay, sizes) = cr_text_layout(&cr, &text, PILL_FONT_SIZE).unwrap();
        self.cached_color = Some(color);
        self.cached_layout = Some(lay);
        self.cached_sizes = Some((sizes.0.max(58.0), sizes.1));
        self.cached_text = Some(text);
        // cr.set_source_rgba(color.0, color.1, color.2, color.3);
        // let sizes = cr_text_rotated_mixed(cr, &*format!("{icon} {text}"), x + rect_width / 2.0, y + rect_height / 2.0, 0.5, 0.5, 0.0, PILL_FONT_SIZE);

    }
}





define_pill_trait_implementer!(PillBattery, {
    battery: Option<crate::battery::BatteryStats>
});

impl PillBattery {
    pub fn new () -> Self {
        PillBattery {
            battery: None,
            cached_layout: None,
            cached_sizes: None,
            cached_text: None,
            cached_color: None
        }
    }

    pub fn update_data (&mut self, cr: &cairo::Context, battery: Option<crate::battery::BatteryStats>) {
        self.battery = battery;

        let green = (0.1, 1.0, 0.2, 1.0);
        // let red = (1.0, 0.1, 0.2, 1.0);

        if let Some(bat) = &self.battery {
            if bat.state != crate::battery::BatteryState::FullyCharged {

                let total_mins = bat.eta_minutes.unwrap_or_default().ceil() as u64;
                let hours = total_mins / 60;
                let minutes = total_mins % 60;

                let eta = match (hours, minutes) {
                    (0, 0) => "0s".to_string(),
                    (0, m) => format!("{}m", m),
                    (h, m) => format!("{}h{}m", h, m)
                };

                let bat_symb: String = match bat.state {
                    crate::battery::BatteryState::Charging => format!("󱐋 {}", eta).into(),
                    crate::battery::BatteryState::Discharging => format!("󰯆 {}", eta).into(),
                    crate::battery::BatteryState::NotCharging => "󱞝".into(),
                    _ => {
                        let slice: &[&str] = ["󰂎", "󰁺", "󰁻", "󰁼", "󰁽", "󰁾", "󰁿", "󰂀", "󰂁", "󰂂", "󰁹"].as_slice();
                        select_icon(0.0, 100.0, bat.percentage, slice).unwrap().into()
                    }
                };
                // if bat.state == crate::battery::BatteryState::Charging {  } else {  };
                let bat_color = match bat.state {
                    crate::battery::BatteryState::Charging => green,
                    crate::battery::BatteryState::Discharging => get_color_gradient(((100.0 - bat.percentage) / 200.0) + 0.5),
                    crate::battery::BatteryState::NotCharging => (0.6, 0.6, 1.0, 1.0),
                    crate::battery::BatteryState::FullyCharged => (0.5, 0.5, 0.8, 0.8),
                    _ => (1.0, 1.0, 1.0, 0.4)
                };
                // cr.set_source_rgba(bat_color.0, bat_color.1, bat_color.2, bat_color.3);
                // let sizes = cr_text_rotated_mixed(&cr, &bat_symb, x, y + rect_height / 2.0, 0.5, 0.5, 0.0, PILL_FONT_SIZE).unwrap();
                let (lay, sizes) = cr_text_layout(&cr, &bat_symb, PILL_FONT_SIZE).unwrap();
                self.cached_layout = Some(lay);
                self.cached_sizes = Some(sizes);
                self.cached_text = Some(bat_symb);
                self.cached_color = Some(bat_color);
                dbg_println!("PillBattery will need a rect {sizes:?}");
                // x += space + sizes2.0 + space;
            } else {
                dbg_println!("PillBattery will need a rect (0.0, 0.0)");
            }
        }
    }
}



pub struct PillWarnings {
    icons: Vec<AlarmIcon>
}

impl PillTrait for PillWarnings {
    fn draw (&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64) {
        // dbg_println!("PillWarnings drawn in rect {sizes:?}");
        dbg_println!("PillWarnings TODO!");
    }

    fn get_current_rect (&self) -> (f64, f64) { (self.icons.len() as f64 * 20.0, 20.0) }
    fn get_desired_rect (&self) -> (f64, f64) { (self.icons.len() as f64 * 20.0, 20.0) }
}

impl PillWarnings {
    pub fn new () -> Self {
        PillWarnings {
            icons: Vec::new()
        }
    }
    pub fn update_data (&mut self, cr: &cairo::Context, icons: Vec<AlarmIcon>) {
        self.icons = icons;
    }
}





/* #[enum_dispatch(PillTrait)]
pub enum PillWrapper {
    Clock1,
    Clock2,
    NoClock
} */