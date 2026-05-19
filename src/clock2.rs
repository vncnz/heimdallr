use cairo::Context;
use chrono::Local;
use chrono::Timelike;

use crate::clock::ClockTrait;
use crate::dbg_println;
use crate::notch_security::NotchTrait;
use crate::utils::rounded_rect_gradient;

pub struct Clock2 {
    // pub(crate) background_surface: Option<cairo::ImageSurface>
}

impl NotchTrait for Clock2 {

    fn is_active (&self) -> bool { true }
    fn get_position (&self) -> crate::utils::Anchor { crate::utils::Anchor::RightFull }
    fn get_height (&self) -> f64 { 0.0 } // unused, RightFull gets all the available height
    fn update_data (&mut self, cr: &Context) -> bool { false } // Clock has no data to be computed and cached
    fn need_redraw(&self) -> bool {
        // TODO: Check if a second has passed
        true
    }

    fn get_width (&self) -> f64 {
        10.0
    }

    fn draw (&mut self, cr: Context, x: f64, y: f64, w: f64, h: f64) { // battery_integrated: Option<crate::battery::BatteryStats>
        let battery_integrated: Option<crate::battery::BatteryStats> = None;
        let wheight = h;
        let right = x + w;
        /*if self.background_surface.is_none() {
            self.draw_clock_background(wheight);
        }
        if let Some(bg) = &self.background_surface {
            cr.set_source_surface(bg, (right - 18) as f64, 0.0).unwrap();
            cr.paint().unwrap();
        }*/

        let now = Local::now();
        let seconds_today =
        now.num_seconds_from_midnight() as f64 + f64::from(now.nanosecond()) / 1_000_000_000.0;
        let y = seconds_today / 86_400.0;
        // let current_hour_ypos = (1.0 - y) * (wheight as f64);
        // dbg_println!("{} {}", y, ypos);

        // let mut interval_time = (current_hour_ypos, wheight as f64);
        // let mut interval_battery: Option<(f64, f64)> = None;
        let mut color_battery: (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0);
        // let mut battery_ypos = wheight as f64;

        let color_green = (0.1, 1.0, 0.2, 1.0);
        let color_red = (1.0, 0.1, 0.2, 1.0);
        let color_full_1 = (1.0, 1.0, 1.0, 1.0);
        let color_full_2 = (0.6, 0.9, 1.0, 1.0);
        let color_half_1 = (1.0, 1.0, 1.0, 0.4);
        let color_half_2 = (0.6, 0.9, 1.0, 0.4);

        let hour_time = y * 24.0;
        let mut hour_battery: Option<f64> = None;
        
        if let Some(bat) = battery_integrated {
            // eta = 10.0;
            if bat.state == crate::battery::BatteryState::Charging {
                color_battery = color_green;
            } else {
                color_battery = color_red;
            };
            hour_battery = Some((bat.eta_minutes.unwrap_or(0.0) / 60.0 + hour_time) % 24.0);
            // dbg_println!("hour_battery {hour_battery:?}");
        }

        let clock_height = wheight as f64 * 0.94;
        let top_shift = (wheight as f64 - clock_height) / 2.0;
        let step_height = clock_height / 24.0;
        let padding = 2.0;
        let xc = (right as f64) - 5.0;
        let w = 4.0;
        let left = xc - w/2.0;

        /* bg */
        /* for drawing_hour in 0..24 {
            // let same = (hour_time / 6.0).ceil() == ((drawing_hour+1) as f64 / 6.0).ceil();
            // let w = if same { 6.0 } else { 2.0 };

            let top = wheight as f64 - drawing_hour as f64 * step_height - step_height;
            let height = step_height;
            // let left = xc - w/2.0;
            /* if drawing_hour % 6 > 0 {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.4);
            } else {
                cr.set_source_rgba(0.6, 0.9, 1.0, 0.4);
            }
            rounded_rect(&cr, xc - w/2.0, top + padding, w, height - (padding*2.0), 2.0);
            cr.fill().unwrap(); */
            rounded_rect_gradient(&cr, left, top, w, height - (padding*2.0), 2.0,
                vec![
                    (0.0, if drawing_hour % 6 > 0 { (1.0, 1.0, 1.0, 0.4) } else { (0.6, 0.9, 1.0, 0.4) })
                ], false
            )
        } */
        /* end bg */

        for drawing_hour in 0..24 {
            let color_full = if drawing_hour % 6 > 0 { color_full_1 } else { color_full_2 };
            let color_half = if drawing_hour % 6 > 0 { color_half_1 } else { color_half_2 };

            let start = drawing_hour as f64;
            let end = start + 1.0;
            let bat = hour_battery.unwrap_or_else(|| hour_time);
            let initial_color = 
                if bat < hour_time && start < bat { color_battery }
                else if start < hour_time { color_full }
                else if start < bat { color_battery }
                else if start > hour_time && bat < hour_time { color_battery }
                else { color_half };
            let mut steps = vec![(0.0, initial_color)];

            if bat > hour_time { // normal case
                if (start..end).contains(&hour_time) {

                    if start < bat { // to green/red
                        let limit = hour_time % 1.0;
                        dbg_println!("0a: h={drawing_hour} limit={limit}");
                        steps.push((limit, color_battery));
                    }
                }
                if (start..end).contains(&bat) { // empty
                    let limit = bat % 1.0;
                    dbg_println!("1a: h={drawing_hour} limit={limit}");
                    steps.push((limit, color_half));
                }
            } else if bat < hour_time { // bat over midnight
                if (start..end).contains(&hour_time) {
                    // to green/red
                    let limit = hour_time % 1.0;
                    dbg_println!("0b: h={drawing_hour} limit={limit}");
                    steps.push((limit, color_battery));
                }
                if (start..end).contains(&bat) {
                    let limit = bat % 1.0;
                    dbg_println!("1b: h={drawing_hour} limit={limit}");
                    steps.push((limit, color_half));
                }
            } else { // no battery
                if (start..end).contains(&hour_time) {
                    let limit = hour_time % 1.0;
                    // dbg_println!("0c: h={drawing_hour} limit={limit}");
                    steps.push((limit, color_half));
                }
            }

            let top = clock_height + top_shift - drawing_hour as f64 * step_height - step_height;
            rounded_rect_gradient(&cr, left, top, w, step_height - (padding*2.0), 2.0, steps, crate::utils::GradientDirection::Vertical, false, None);
        }
    }
}

impl Clock2 {
    /* fn draw_clock_background(&mut self, wheight: i32) {
        let width = 18;
        let height = wheight;
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();

        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        cr.paint().unwrap();

        self.background_surface = Some(surface);
    } */
}