use std::time::Instant;

use cairo::{Context, FontSlant};
use chrono::Local;

use crate::{security::MicCameraStatus, utils::{Anchor, cr_text_aligned, cr_text_rotated, rounded_rect_gradient}};

// TODO: improve animation system with easing logic and with a robust way to handle last drawing!

pub enum Easing {
    #[allow(unused)]
    Linear,
    #[allow(unused)]
    Smooth,
    #[allow(unused)]
    Smoother,
    EaseOutCubic,
}
fn ease(e: Easing, t: f64) -> f64 {
    let x = t.clamp(0.0, 1.0);
    match e {
        Easing::Linear => x,
        Easing::Smooth => x * x * (3.0 - 2.0 * x),
        Easing::Smoother => x * x * x * (x * (x * 6.0 - 15.0) + 10.0),
        Easing::EaseOutCubic => 1.0 - (1.0 - x).powi(3),
    }
}

// #[derive(Default)]
struct AnimationNew {
    start_value: f64,
    end_value: f64,
    duration: u64,
    start_time: Instant
}
impl AnimationNew {
    pub fn get_current_value (self: &Self) -> f64 {
        let elapsed = self.start_time.elapsed().as_millis() as u64;
        if elapsed >= self.duration {
            self.end_value
        } else {
            let progress = elapsed as f64 / self.duration as f64;
            // let linear_value = self.start_value + (self.end_value - self.start_value) * progress;
            let eased = ease(Easing::EaseOutCubic, progress);
            self.start_value + (self.end_value - self.start_value) * eased
        }
    }

    pub fn animate_property (self: &mut Self, end_value: f64, duration: u64) {
        self.start_value = self.get_current_value();
        self.end_value = end_value;
        self.duration = duration;
        self.start_time = Instant::now();
    }

    pub fn new (value: f64) -> Self {
        AnimationNew { start_value: 0.0, end_value: value, duration: 0, start_time: Instant::now() }
    }

    pub fn is_active (&self) -> bool {
        self.start_time.elapsed().as_millis() as u64  <= self.duration
    }
}








pub trait NotchTrait {
    fn new () -> Self;
    fn update_data (&mut self, cr: &Context) -> bool;
    fn draw (&mut self, cr: Context, x: f64, y: f64);
    fn need_redraw(&self) -> bool;
    // fn get_area (&self) -> ReservedSpace;
    fn get_position (&self) -> Anchor;
    fn get_width (&self) -> f64;
    fn get_height (&self) -> f64;
    fn is_active (&self) -> bool;
}

pub struct SecurityNotch {
    pub(crate) security: crate::security::MicCameraStatus,
    pub(crate) last_width: f64,
    pub(crate) last_text: String,
    height_animator: AnimationNew
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
            last_text: "".to_string(),
            height_animator: AnimationNew::new(0.0)
        }
    }

    fn is_active (&self) -> bool {
        self.height_animator.get_current_value() > 0.0
    }

    fn need_redraw(&self) -> bool {
        self.height_animator.is_active()
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
        12.0 * self.height_animator.get_current_value()
    }

    fn update_data(&mut self, cr: &Context) -> bool {
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
            self.height_animator.animate_property(
                if self.last_text.is_empty() { 0.0 } else { 1.0 },
                200
            );
            if !self.last_text.is_empty() {
                cr.set_font_size(10.0);
                cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
                if let Ok(ext) = cr.text_extents(&self.last_text) {
                    self.last_width = ext.width() + 6.0;
                } else {
                    self.last_width = 0.0;
                }
            }
            true
        } else {
            false
        }
    }

    // (x,y) depends on declared anchor
    fn draw (&mut self, cr: Context, x: f64, y: f64) {
        // let draw_mic = self.security.mic_active.len() > 0;
        // let draw_cam = self.security.camera_active.len() > 0;
        // if draw_mic || draw_cam {
        let anim_value = self.height_animator.get_current_value();
            let r = 4.0;
            let mic_color = (1.0, 0.58, 0.0, anim_value);
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
            rounded_rect_gradient(&cr, x - (self.last_width / 2.0), y, self.last_width, 12.0 * anim_value, r, steps, crate::utils::GradientDirection::Horizontal, false, None);

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
        // }
    }
}


pub struct ClockNotch {
    pub(crate) last_height: f64,
    pub(crate) last_text: String
}
impl NotchTrait for ClockNotch {

    fn new () -> Self {
        ClockNotch {
            last_height: 0.0,
            last_text: "".to_string()
        }
    }

    fn is_active (&self) -> bool {
        true
    }

    fn need_redraw(&self) -> bool {
        true
    }

    fn get_position (&self) -> Anchor {
        Anchor::LeftFloating
    }
    fn get_width (&self) -> f64 {
        0.0
    }
    fn get_height (&self) -> f64 {
        self.last_height + 2.0
    }

    fn update_data(&mut self, cr: &Context) -> bool {
    
        let now = Local::now();
        let text = now.format("%H:%M").to_string();
        self.last_text = text;
        /* self.animator.animate_property(
            &self.frame_model,
            AnimationKey::SecurityNotchRatio,
            if self.last_text.is_empty() { 0.0 } else { 1.0 },
            200
        ); */
        cr.set_font_size(10.0);
        cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
        if let Ok(ext) = cr.text_extents(&self.last_text) {
            self.last_height = ext.width() + 6.0;
        } else {
            self.last_height = 0.0;
        }
        true
    }

    // (x,y) depends on declared anchor
    fn draw (&mut self, cr: Context, x: f64, y: f64) {

        cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(18.0);

        cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        // let (_w_hours, h_hours) = cr_text_aligned(cr.clone(), hours, xc, y, 0.5, 0.0);
        let (w_time, h_time) = cr_text_rotated(&cr, &self.last_text.to_string(), 0.0, y, 0.0, 0.0, -90.0).ok().unwrap_or((0.0, 0.0));
        // top += h_time + 2.0;
    }
}