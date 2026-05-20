use cairo::{Context, FontSlant};
use chrono::Local;

use crate::clock::ClockTrait;
use crate::utils::{cr_text_aligned, cr_text_rotated, cr_text_rotated_mixed};

pub struct Clock4 {
    // pub(crate) background_surface: Option<cairo::ImageSurface>
}

impl Clock4 {
    pub fn new () -> Self {
        Clock4 {}
    }
}

impl ClockTrait for Clock4 {

    fn get_reserved_width (&self) -> f64 {
        8.0
    }

    fn get_reserved_height (&self) -> f64 {
        200.0
    }

    fn draw (&mut self, cr: Context, x: f64, y: f64, w: f64, _h: f64, battery_integrated: Option<crate::battery::BatteryStats>) -> (f64, f64) {
        let xc = x + w / 2.0;
        let mut top = y;

        let now = Local::now();
        // println!("{}", now.format("%Y-%m-%d][%H:%M:%S"));
        // let hours = now.format("%H").to_string();
        // let minutes = now.format("%M").to_string();
        let time = now.format("%H:%M").to_string();
        
        cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(18.0);

        cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        // let (_w_hours, h_hours) = cr_text_aligned(cr.clone(), hours, xc, y, 0.5, 0.0);
        let (w_time, h_time) = cr_text_rotated(&cr, &time.to_string(), xc, y, 0.0, 0.5, -90.0).ok().unwrap_or((0.0, 0.0));
        top += h_time + 2.0;

        // let size = cr_text_rotated_mixed(&cr, "󱐋 Test", xc, y-70.0, 0.0, 0.5, -90.0, 16.0).ok();
        // eprintln!("Size: {:?}\n", size);

        // cr.set_font_size(16.0);
        // let (_w_minutes, h_minutes) = cr_text_aligned(cr.clone(), minutes, xc, top, 0.5, 0.0);
        // top += h_minutes + 2.0;

        if let Some(bat) = battery_integrated {
            if bat.state == crate::battery::BatteryState::Charging || bat.state == crate::battery::BatteryState::Discharging {
                let color: (f64, f64, f64, f64);
                let symbol: String;
                if bat.state == crate::battery::BatteryState::Charging {
                    color = (0.1, 1.0, 0.2, 1.0);
                    symbol = "󱐋".into();
                } else {
                    color = (1.0, 0.2, 0.3, 1.0);
                    symbol = "󰯆".into();
                };

                let minutes = bat.eta_minutes.unwrap_or(0.0);

                // let bat_symb: String = if bat.state == crate::battery::BatteryState::Charging { "󱐋".into() } else { "󰯆".into() };
                let text = if minutes > 90.0 {
                    format!("{} {}h", symbol, (minutes / 60.0).round())
                } else if minutes > 1.0 {
                    format!("{} {}m", symbol, minutes.round())
                } else if minutes > 0.0 {
                    format!("{} now", symbol)
                } else {
                    format!("{} unknown", symbol)
                };

                cr.set_source_rgba(color.0, color.1, color.2, color.3);
                // cr.set_font_size(16.0);
                //let (_w_eta, h_eta) = cr_text_aligned(cr.clone(), text, xc, top, 0.5, 0.0);
                let (_w_eta, h_eta) = cr_text_rotated_mixed(&cr, &text.to_string(), xc, y-70.0, 0.0, 0.5, -90.0, 16.0).ok().unwrap_or_default();
                top += h_eta;
            }

            eprintln!("x: {}, xc: {}, w: {}, w_time: {}, h_time: {}", x, xc, w, w_time, h_time);

            // dbg_println!("Battery moving");
            /*let bat_symb: String = if bat.state == crate::battery::BatteryState::Charging { "󱐋".into() } else { "󰯆".into() };
            let font_size: f64;
            let color: (f64, f64, f64, f64);
            if bat.state == crate::battery::BatteryState::Charging {
                font_size = 20.0;
                color = (0.1, 1.0, 0.2, 1.0);
            } else {
                font_size = 14.0;
                color = (1.0, 0.1, 0.2, 1.0);
            };
            if let Some(eta) = bat.eta_minutes {
                let bpos = (ypos - (eta / 1440.0 * wheight as f64) + wheight as f64) % wheight as f64;
                
                /* Border */
                cr.set_font_size(font_size + 2.0);
                cr.set_source_rgba(0.1,0.1,0.1,1.0);
                cr_text_aligned(cr.clone(), bat_symb.clone(), right as f64 - 8.0 - 1.0, bpos, 1.0, 0.5);
                /* end */
                
                cr.set_font_size(font_size);
                let (r,g,b,a) = color;
                cr.set_source_rgba(r,g,b,a);
                cr_text_aligned(cr.clone(), bat_symb, right as f64 - 8.0, bpos, 1.0, 0.5);
            } else {
                // cr_text_aligned(cr.clone(), bat_symb, right as f64, 0.0, 1.0, 0.5);
            }*/
            
            // let extents = cr.text_extents(text).unwrap();
        } else {
            // dbg_println!("No battery info/moving");
        }
        (w, top)
    }
}
