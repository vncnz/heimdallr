use std::cmp::min;

use cairo::{Context, FontSlant};
use chrono::Local;
use chrono::Timelike;

use crate::clock::ClockTrait;
use crate::utils::rounded_rect;
use crate::utils::{cr_text_aligned, rounded_big_hole};

pub struct Clock2 {
    pub(crate) background_surface: Option<cairo::ImageSurface>
}

impl Clock2 {
    pub fn new () -> Self {
        Clock2 {
            background_surface: None
        }
    }
}

impl ClockTrait for Clock2 {

    fn get_reserved_width (&self) -> f64 {
        10.0
    }

    fn draw (&mut self, cr: Context, wheight: i32, right: u32, battery_recharging: Option<bool>, battery_eta: Option<f64>) {
        if self.background_surface.is_none() {
            self.draw_clock_background(wheight);
        }
        if let Some(bg) = &self.background_surface {
            cr.set_source_surface(bg, (right - 18) as f64, 0.0).unwrap();
            cr.paint().unwrap();
        }

        let now = Local::now();
        let seconds_today =
        now.num_seconds_from_midnight() as f64 + f64::from(now.nanosecond()) / 1_000_000_000.0;
        let y = seconds_today / 86_400.0;
        let current_hour_ypos = (1.0 - y) * (wheight as f64);
        // dbg_println!("{} {}", y, ypos);

        // let mut interval_time = (current_hour_ypos, wheight as f64);
        // let mut interval_battery: Option<(f64, f64)> = None;
        let mut color: (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0);
        // let mut battery_ypos = wheight as f64;


        let hour_time = y * 24.0;
        let mut hour_battery: Option<f64> = None;
        
        if let (Some(rec), Some(eta)) = (battery_recharging, battery_eta) {
            if rec {
                color = (0.1, 1.0, 0.2, 1.0);
            } else {
                color = (1.0, 0.1, 0.2, 1.0);
            };
            /* battery_ypos = (current_hour_ypos - (eta / 1440.0 * wheight as f64) + wheight as f64) % wheight as f64;
            if battery_ypos < current_hour_ypos {
                interval_battery = Some((battery_ypos, current_hour_ypos));
            } else if battery_ypos > current_hour_ypos {
                interval_battery = Some((wheight as f64, battery_ypos));
                interval_time = (current_hour_ypos, battery_ypos);
            } */
            hour_battery = Some((eta / 60.0 + hour_time) % 24.0);
            eprintln!("hour_battery {hour_battery:?}");
        }


        let step_height = wheight as f64 / 24.0;
        let padding = 2.0;
        let xc = (right as f64) - 5.0;
        let w = 4.0;

        /* bg */
        for drawing_hour in 0..24 {
            // let same = (hour_time / 6.0).ceil() == ((drawing_hour+1) as f64 / 6.0).ceil();
            // let w = if same { 6.0 } else { 2.0 };

            let top = wheight as f64 - drawing_hour as f64 * step_height - step_height;
            let height = step_height;
            if drawing_hour % 6 > 0 {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.4);
            } else {
                cr.set_source_rgba(0.6, 0.9, 1.0, 0.4);
            }
            rounded_rect(&cr, xc - w/2.0, top + padding, w, height - (padding*2.0), 2.0);
            cr.fill().unwrap();
        }
        /* end bg */

        for drawing_hour in 0..24 {
            // let same = (hour_time / 6.0).ceil() == ((drawing_hour+1) as f64 / 6.0).ceil();
            // let w = if same { 6.0 } else { 2.0 };

            let mut top: Option<f64> = None;
            let mut height: f64 = 0.0;
            if (drawing_hour as f64 + 1.0) < hour_time { // White box
                top = Some(wheight as f64 - drawing_hour as f64 * step_height - step_height);
                height = step_height;
            } else if (drawing_hour as f64) < hour_time {
                top = Some(wheight as f64 - hour_time * step_height);
                height = (hour_time - drawing_hour as f64) * step_height;
            }

            if let Some(t) = top {
                // let (r,g,b,a) = color;
                // cr.set_source_rgba(r,g,b,a);
                // cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                if drawing_hour % 6 > 0 {
                    cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                } else {
                    cr.set_source_rgba(0.6, 0.9, 1.0, 1.0);
                }
                rounded_rect(&cr, xc - w/2.0, t + padding, w, height - (padding*2.0), 2.0);
                cr.fill().unwrap();
            }


            if let Some(hour_bat) = hour_battery && drawing_hour as f64 > (hour_time-1.0) {
                // red/green box
                if hour_time > drawing_hour as f64 {
                    let remove = if hour_bat < (drawing_hour + 1) as f64 { ((drawing_hour + 1) as f64 - hour_bat)*step_height } else { 0.0 };
                    top = Some(wheight as f64 - drawing_hour as f64 * step_height - step_height + remove);
                    height = step_height * (drawing_hour as f64 + 1.0 - hour_time) - remove;
                    // eprintln!("case 0 h={drawing_hour} height={height}");
                } else if (drawing_hour as f64 + 1.0) < hour_bat {
                    top = Some(wheight as f64 - drawing_hour as f64 * step_height - step_height);
                    height = step_height;
                    // eprintln!("case 1 h={drawing_hour} height={height}");
                } else if (drawing_hour as f64) < hour_bat {
                    top = Some(wheight as f64 - hour_bat * step_height);
                    height = (hour_bat - drawing_hour as f64) * step_height;
                    // eprintln!("case 2 h={drawing_hour} height={height}");
                } else {
                    // eprintln!("case 3 h={drawing_hour}");
                    top = None;
                }

                if let Some(t) = top {
                    let (r,g,b,a) = color;
                    cr.set_source_rgba(r,g,b,a);
                    // cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                    rounded_rect(&cr, xc - w/2.0, t + padding, w, height - (padding*2.0), 2.0);
                    cr.fill().unwrap();
                }
            }
        }
    }
}

impl Clock2 {
    fn draw_clock_background(&mut self, wheight: i32) {
        let width = 18;
        let height = wheight;
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();

        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        cr.paint().unwrap();

        // cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);

        /* for h in 1..24 {
            let y = (1.0 - (h as f64 / 24.0)) * height as f64;
            let mut symb = "".to_string(); // (h % 10).to_string();
            let mut x = 15.0;
            if h % 6 == 0 {
                cr.set_source_rgba(0.6, 0.9, 1.0, 1.0);
                cr.set_font_size(20.0);
                x = 12.0;
                cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);
                symb = "".into();
            } else if h % 3 == 0 {
                cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                cr.set_font_size(12.0);
                cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Bold);
            } else {
                cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                cr.set_font_size(10.0);
                cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
            }
            cr_text_aligned(cr.clone(), symb, x, y, 0.5, 0.5);
        } */

        self.background_surface = Some(surface);
    }
}