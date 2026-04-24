use cairo::{Context, FontSlant};
use chrono::Local;
use chrono::Timelike;

use crate::clock::ClockTrait;
use crate::utils::cr_text_aligned;

pub struct Clock1 {
    pub(crate) background_surface: Option<cairo::ImageSurface>
}

impl Clock1 {
    pub fn new () -> Self {
        Clock1 {
            background_surface: None
        }
    }
}

impl ClockTrait for Clock1 {

    fn get_reserved_width (&self) -> f64 {
        8.0
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
        let ypos = (1.0 - y) * (wheight as f64);
        // dbg_println!("{} {}", y, ypos);

        /* Border */
        cr.set_source_rgba(0.1, 0.1, 0.1, 1.0);
        cr.move_to((right - 24u32) as f64 + 1.0, (1.0 - y) * (wheight as f64) - 1.0);
        cr.set_font_size(17.0);
        // cr.show_text("");
        cr_text_aligned(cr.clone(), "".into(), right as f64 - 5.0, ypos, 1.0, 0.0);
        /* end */

        cr.set_source_rgba(1.0, 0.1, 0.2, 1.0);
        cr.move_to((right - 24u32) as f64, (1.0 - y) * (wheight as f64));
        cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(15.0);
        cr_text_aligned(cr.clone(), "".into(), right as f64 - 5.0, ypos, 1.0, 0.0);

        if let Some(rec) = battery_recharging {
            // dbg_println!("Battery moving");
            let bat_symb: String = if rec { "󱐋".into() } else { "󰯆".into() };
            let font_size: f64;
            let color: (f64, f64, f64, f64);
            if rec {
                font_size = 20.0;
                color = (0.1, 1.0, 0.2, 1.0);
            } else {
                font_size = 14.0;
                color = (1.0, 0.1, 0.2, 1.0);
            };
            if let Some(eta) = battery_eta {
                let bpos = (ypos - (eta / 1440.0 * wheight as f64) + wheight as f64) % wheight as f64;
                
                /* Border */
                cr.set_font_size(font_size + 2.0);
                cr.set_source_rgba(0.1,0.1,0.1,1.0);
                cr_text_aligned(cr.clone(), bat_symb.clone(), right as f64 - 7.0 + 1.0, bpos, 1.0, 0.5);
                /* end */
                
                cr.set_font_size(font_size);
                let (r,g,b,a) = color;
                cr.set_source_rgba(r,g,b,a);
                cr_text_aligned(cr.clone(), bat_symb, right as f64 - 7.0, bpos, 1.0, 0.5);
            } else {
                cr_text_aligned(cr.clone(), bat_symb, right as f64, 0.0, 1.0, 0.5);
            }
            
            // let extents = cr.text_extents(text).unwrap();
        } else {
            // dbg_println!("No battery info/moving");
        }
    }
}

impl Clock1 {
    fn draw_clock_background(&mut self, wheight: i32) {
        let width = 18;
        let height = wheight;
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();

        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        cr.paint().unwrap();

        // cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);

        for h in 1..24 {
            let y = (1.0 - (h as f64 / 24.0)) * height as f64;
            let mut symb = (h % 10).to_string();
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
        }

        self.background_surface = Some(surface);
    }
}