use cairo::{Context, FontSlant};

use crate::{security::MicCameraStatus, utils::{Anchor, cr_text_aligned, rounded_rect_gradient}};



pub trait NotchTrait {
    fn new () -> Self;
    fn update_data (&mut self, cr: &Context) -> Option<f64>;
    fn draw (&mut self, cr: Context, x: f64, y: f64, anim_ratio: f64);
    // fn get_area (&self) -> ReservedSpace;
    fn get_position (&self) -> Anchor;
    fn get_width (&self) -> f64;
    fn get_height (&self) -> f64;
}

pub struct SecurityNotch {
    pub(crate) security: crate::security::MicCameraStatus,
    pub(crate) last_width: f64,
    pub(crate) last_text: String
}

impl SecurityNotch {
    fn build_security_text (&self) -> String {
        /* (
            self.security.mic_active.clone().into_iter().map(|s| format!("MIC {s}")).collect::<Vec<_>>().join("  ·  "), 
            self.security.camera_active.clone().into_iter().map(|s| format!("CAM {s}")).collect::<Vec<_>>().join("  ·  ")
        ) */
       self.security.mic_active.clone().into_iter().map(|s| format!("MIC {s}")).chain(self.security.camera_active.clone().into_iter().map(|s| format!("CAM {s}"))).collect::<Vec<_>>().join("  ·  ")
    }
}

impl NotchTrait for SecurityNotch {

    
    fn new () -> Self {
        SecurityNotch {
            security: MicCameraStatus { mic_active: vec!(), camera_active: vec!(), pristine: false },
            last_width: 0.0,
            last_text: "".to_string()
        }
    }

    /* fn get_area(&self) -> ReservedSpace {
        ReservedSpace { anchor: Anchor::TopCenter, width: self.last_width, height: 14.0 }
    } */

    fn get_position (&self) -> Anchor {
        Anchor::TopCenter
    }
    fn get_width (&self) -> f64 {
        self.last_width + 2.0
    }
    fn get_height (&self) -> f64 {
        12.0
    }

    fn update_data(&mut self, cr: &Context) -> Option<f64> {
        if self.security.pristine {
            self.security.pristine = false;
            let text = self.build_security_text();
            self.last_text = text;
            /* self.animator.animate_property(
                &self.frame_model,
                AnimationKey::SecurityNotchRatio,
                if self.last_text.is_empty() { 0.0 } else { 1.0 },
                200
            ); */
            if !self.last_text.is_empty() {
                cr.set_font_size(10.0);
                cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
                if let Ok(ext) = cr.text_extents(&self.last_text) {
                    self.last_width = ext.width() + 6.0;
                } else {
                    self.last_width = 0.0;
                }
            }
            Some(if self.last_text.is_empty() { 0.0 } else { 1.0 })
        } else {
            None
        }
    }

    // x0: (self.width as f64 - self.last_width) / 2.0, y0: 0.0
    // (x,y) depends on declared anchor
    fn draw (&mut self, cr: Context, x: f64, y: f64, anim_ratio: f64) {
        let draw_mic = self.security.mic_active.len() > 0;
        let draw_cam = self.security.camera_active.len() > 0;
        if draw_mic || draw_cam {
            let r = 4.0;
            let mic_color = (1.0, 0.58, 0.0, anim_ratio);
            // let cam_color = (0.2, 0.78, 0.35, 1.0);
            /* let x = 1.0;
            let y = 1.0;
            let w = 10.0;
            let h = 10.0;
            let steps = if draw_mic && draw_cam { vec![(0.0, mic_color), (1.0, cam_color)] } else if draw_mic { vec![(0.0, mic_color)] } else { vec![(0.0, cam_color)] };
            rounded_rect_gradient(&cr, x, y, w, h, r, steps, crate::utils::GradientDirection::Horizontal, true, None); */

            cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
            cr.set_font_size(10.0);

            let steps = vec![(0.0, (mic_color.0, mic_color.1, mic_color.2, mic_color.3))];
            rounded_rect_gradient(&cr, x - (self.last_width / 2.0), y, self.last_width, 12.0, r, steps, crate::utils::GradientDirection::Horizontal, false, None);

            cr.set_source_rgba(0.0, 0.0, 0.0 ,1.0);
            // cr.move_to(14.0, 9.0);
            // cr.show_text(&mic).unwrap();
            cr_text_aligned(cr.clone(), self.last_text.clone(), x, y + 2.0, 0.5, 0.0);
            /* for app in self.security.mic_active.clone().into_iter() {
                cr.move_to(14.0, 9.0);
                cr.show_text(&app).unwrap();
                // let w = cr.text_extents(&text).unwrap().width();
                // cr_text_aligned(cr.clone(), app.into(), self.width / 2.0, 0.5, 0.0);
            } */
        }
    }
}