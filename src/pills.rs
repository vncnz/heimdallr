use cairo::Context;
use chrono::Local;

use crate::{countdown::Countdown, dbg_println, utils::{cr_text_aligned, cr_text_rotated_mixed, get_color_gradient}};
// use enum_dispatch::enum_dispatch;

// use crate::{clock1::Clock1, clock2::Clock2};

// #[enum_dispatch]
pub trait PillTrait {
    // fn new () -> Self;
    fn draw (&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64);
    fn get_current_rect (&self) -> (f64, f64);
    fn get_desired_rect (&self) -> (f64, f64);
}

pub struct PillClock {}
impl PillTrait for PillClock {

    fn draw (&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64) {

        let date = Local::now();
        let text = date.format("%H:%M").to_string();

        cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        let sizes = cr_text_aligned(cr.clone(), text.into(), x + rect_width / 2.0, y + rect_height / 2.0, 0.5, 0.5);
        dbg_println!("PillClock drawn in rect {sizes:?}");
        // x += space + sizes.0 + space;
    }

    fn get_current_rect (&self) -> (f64, f64) { (45.0, 20.0) }
    fn get_desired_rect (&self) -> (f64, f64) { (45.0, 20.0) }
}

impl PillClock {
    pub fn new () -> Self {
        PillClock {}
    }
}






pub struct PillCountdown {
    countdown: Countdown
}
impl PillTrait for PillCountdown {

    fn draw (&mut self, cr: &Context, rect_width: f64, rect_height: f64, x: f64, y: f64) {

        let (status, text) = self.countdown.format_custom_duration();
        let w = if status { 1.0 } else { self.countdown.get_warning() };
        let icon = if status { "󱫌" } else { "󱫡" };
        let color = get_color_gradient(w);


        cr.set_source_rgba(color.0, color.1, color.2, color.3);
        let sizes = cr_text_rotated_mixed(cr, &*format!("{icon} {text}"), x + rect_width / 2.0, y + rect_height / 2.0, 0.5, 0.5, 0.0, 16.0);
        dbg_println!("PillCountdown drawn in rect {sizes:?}");
        // x += space + sizes.0 + space;
    }

    fn get_current_rect (&self) -> (f64, f64) { (80.0, 20.0) }
    fn get_desired_rect (&self) -> (f64, f64) { (80.0, 20.0) }
}

impl PillCountdown {
    pub fn new (countdown: Countdown) -> Self {
        PillCountdown {
            countdown
        }
    }
}


/* #[enum_dispatch(PillTrait)]
pub enum PillWrapper {
    Clock1,
    Clock2,
    NoClock
} */