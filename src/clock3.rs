use cairo::{Context, FontSlant};
use chrono::{Local, Timelike};

use crate::clock::ClockTrait;
use crate::utils::cr_text_aligned;

use std::f64::consts::PI;

/* #[derive(Clone, Copy)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
} */

type Color = (f64, f64, f64, f64);

/// Configurazione per il singolo spicchio
#[derive(Clone, Copy)]
pub struct SliceConfig {
    pub fill_color: Color,
    /// Se Some(colore), disegna il bordo trapezoidale esterno
    pub border_color: Option<Color>,
}

/// Disegna un esagono compatto con bordi trapezoidali integrati per icone di piccole dimensioni.
///
/// # Parametri
/// - `outer_radius`: Il raggio totale dell'icona (es. 11.0 per un diametro di 22px).
/// - `spacing`: Distanza di offset per separare i 6 spicchi tra loro.
/// - `border_thickness`: Spessore (in pixel) del trapezio del bordo.
/// - `gap_thickness`: Lo spazio vuoto tra il bordo trapezoidale e lo spicchio interno.
pub fn draw_micro_hexagon(
    cr: &cairo::Context,
    center_x: f64,
    center_y: f64,
    outer_radius: f64,
    spacing: f64,
    border_thickness: f64,
    gap_thickness: f64,
    slices: [SliceConfig; 6],
) {
    let angle_step = PI / 3.0;

    // Calcoliamo i raggi dinamici per evitare sovrapposizioni
    let r_border_outer = outer_radius;
    let r_border_inner = (r_border_outer - border_thickness).max(0.0);
    let r_slice_max = (r_border_inner - gap_thickness).max(0.0);

    for i in 0..6 {
        let start_angle = i as f64 * angle_step;
        let end_angle = (i + 1) as f64 * angle_step;

        // Direzione del distanziamento (offset)
        let mid_angle = start_angle + (angle_step / 2.0);
        let offset_x = mid_angle.cos() * spacing;
        let offset_y = mid_angle.sin() * spacing;

        let s_center_x = center_x + offset_x;
        let s_center_y = center_y + offset_y;

        let config = &slices[i];

        // 1. DISEGNO DEL BORDO ESTERNO (Se abilitato)
        // Se il bordo esiste, lo disegnamo come un trapezio isolato
        if let Some(b_color) = config.border_color {
            // Vertici esterni del trapezio
            let bo1_x = s_center_x + start_angle.cos() * r_border_outer;
            let bo1_y = s_center_y + start_angle.sin() * r_border_outer;
            let bo2_x = s_center_x + end_angle.cos() * r_border_outer;
            let bo2_y = s_center_y + end_angle.sin() * r_border_outer;

            // Vertici interni del trapezio
            let bi1_x = s_center_x + start_angle.cos() * r_border_inner;
            let bi1_y = s_center_y + start_angle.sin() * r_border_inner;
            let bi2_x = s_center_x + end_angle.cos() * r_border_inner;
            let bi2_y = s_center_y + end_angle.sin() * r_border_inner;

            // Tracciamento del trapezio
            cr.move_to(bo1_x, bo1_y);
            cr.line_to(bo2_x, bo2_y);
            cr.line_to(bi2_x, bi2_y);
            cr.line_to(bi1_x, bi1_y);
            cr.close_path();

            cr.set_source_rgba(b_color.0, b_color.1, b_color.2, b_color.3);
            let _ = cr.fill();
        }

        // 2. DISEGNO DEL CORPO DELLO SPICCHIO
        // Se c'è il bordo, il raggio massimo sarà ridotto (r_slice_max),
        // altrimenti se non c'è il bordo usiamo tutto lo spazio fino a r_border_inner per coerenza geometrica
        let current_r_max = if config.border_color.is_some() {
            r_slice_max
        } else {
            r_border_inner // mantiene la forma allineata al limite interno del potenziale bordo
        };

        let v1_x = s_center_x + start_angle.cos() * current_r_max;
        let v1_y = s_center_y + start_angle.sin() * current_r_max;
        let v2_x = s_center_x + end_angle.cos() * current_r_max;
        let v2_y = s_center_y + end_angle.sin() * current_r_max;

        cr.move_to(s_center_x, s_center_y);
        cr.line_to(v1_x, v1_y);
        cr.line_to(v2_x, v2_y);
        cr.close_path();

        cr.set_source_rgba(config.fill_color.0, config.fill_color.1, config.fill_color.2, config.fill_color.3);
        let _ = cr.fill();
    }
}

pub struct Clock3 {
    // pub(crate) background_surface: Option<cairo::ImageSurface>
}

impl Clock3 {
    pub fn new () -> Self {
        Clock3 {}
    }
}

impl ClockTrait for Clock3 {

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
        let hours = now.format("%H").to_string();
        let minutes = now.format("%M").to_string();
        
        cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(18.0);

        cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        let (_w_hours, h_hours) = cr_text_aligned(cr.clone(), hours, xc, y, 0.5, 0.0);
        top += h_hours + 2.0;

        // eprint!("{}+{} ", w_hours, y);

        cr.set_line_width(1.0);
        cr.new_sub_path();
        cr.move_to(x + 4.0, top);
        cr.line_to(x + w - 4.0, top);
        cr.stroke();
        top += 2.0;

        cr.set_font_size(16.0);
        let (_w_minutes, h_minutes) = cr_text_aligned(cr.clone(), minutes, xc, top, 0.5, 0.0);
        top += h_minutes + 2.0;

        let mut eta_hours = 0.0;

        let mut color: (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0);
        if let Some(bat) = battery_integrated {
            if bat.state == crate::battery::BatteryState::Charging || bat.state == crate::battery::BatteryState::Discharging {
                eta_hours = bat.eta_minutes.unwrap_or_default() / 60.0;
                if bat.state == crate::battery::BatteryState::Charging {
                    color = (0.1, 1.0, 0.2, 1.0);
                } else {
                    color = (1.0, 0.2, 0.3, 1.0);
                };

                let minutes = bat.eta_minutes.unwrap_or(0.0);

                // let bat_symb: String = if bat.state == crate::battery::BatteryState::Charging { "󱐋".into() } else { "󰯆".into() };
                let text = if minutes > 90.0 {
                    format!("{}h", (minutes / 60.0).round())
                } else if minutes > 0.0 {
                    format!("{}", minutes.round())
                } else if bat.state == crate::battery::BatteryState::Charging {
                    "󱐋".into()
                } else {
                    "󰯆".into()
                };

                cr.set_source_rgba(color.0, color.1, color.2, color.3);
                cr.set_font_size(16.0);
                let (_w_eta, h_eta) = cr_text_aligned(cr.clone(), text, xc, top, 0.5, 0.0);
                top += h_eta;
            }

            // eta_hours = 3.4;

            let outer_radius = 11.0;
            let spacing = 1.0;
            let border_thickness = 2.0;
            let gap_thickness = 1.0;
            let white = (1.0, 1.0, 1.0, 1.0);
            /* let middle = match self.config.frame_color {
                FrameColor::Rgba(r, g, b, a) => Some((r, g, b, a)),
                _ => (1.0, 1.0, 1.0, 0.6)
            }; */
            let gray = (1.0, 1.0, 1.0, 0.2);
            let green = (0.2, 1.0, 0.4, 0.8);

            let current_hour = now.hour() as f64 % 6.0;

            let mut slices_hours = [SliceConfig { fill_color: gray, border_color: None }; 6];
            for (i, slice) in slices_hours.iter_mut().enumerate() {
                let eta_ends = current_hour + eta_hours;
                slice.fill_color = if (i as f64) < current_hour { white }
                                   else if (i as f64) < eta_ends { color }
                                   else { gray };
                
                slice.border_color = if (i as f64 + 6.5) < eta_ends { Some(color) }
                                     else { None };
            }
            draw_micro_hexagon(&cr, xc, top + 20.0, outer_radius, spacing, border_thickness, gap_thickness, slices_hours);
            
            let eta_tens = eta_hours * 6.0;
            let current_tents = now.minute() as f64 / 10.0;
            let mut slices_minutes = [SliceConfig { fill_color: gray, border_color: None }; 6];
            for (i, slice) in slices_minutes.iter_mut().enumerate() {
                let eta_ends = current_tents + eta_tens;
                slice.fill_color = if (i as f64 + 1.0) < current_tents { white }
                                   else if (i as f64 + 1.0) < eta_ends { color }
                                   else { gray };
                
                // slice.border_color = if (i as f64 + 6.5) < eta_ends { Some(green) }
                //                      else { None };
            }
            draw_micro_hexagon(&cr, xc, top + 50.0, outer_radius, spacing, border_thickness, gap_thickness, slices_minutes);

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
