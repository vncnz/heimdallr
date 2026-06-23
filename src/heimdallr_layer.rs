use smithay_client_toolkit::{
    compositor::CompositorHandler, delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm, output::{OutputHandler, OutputState}, registry::{ProvidesRegistryState, RegistryState}, registry_handlers, shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure}, shm::{Shm, ShmHandler, slot::{Buffer, SlotPool}}
};
use wayland_client::{Connection, QueueHandle, protocol::{wl_compositor, wl_region, wl_shm}};
use cairo::{Context, Format, ImageSurface};

use std::{num::NonZeroU32, time::{Duration, Instant}};

use smithay_client_toolkit::shell::WaylandSurface;

use std::collections::HashMap;
use cairo::FontSlant;

use wayland_client::Dispatch;
use colored::Colorize;

use crate::{clock::ClockTrait, config::FrameColor, countdown::Countdown, data::BatteryDevice, dbg_println, notifications::Notification, pills::{PillBattery, PillClock, PillCountdown, PillTrait, PillWarnings}, utils::{Anchor, AnimationKey, Animator, FrameModel, ReservedSpace, cr_text_aligned, cr_text_rotated, cr_text_rotated_mixed, draw_smart_border, get_color_gradient, log_to_file, rounded_rect_gradient, select_icon}};

#[derive(PartialEq)]
pub enum IconChange {
    Added,
    Changed,
    // Removed,
    None
}
#[derive(Clone)]
pub struct AlarmIcon {
    symbol: String,
    color: (f64, f64, f64, f64), // RGBA
    warn: f64,
    info: Option<String>
}

static mut AVG_DUR: u128 = 0;
static mut AVG_CNT: i64 = -5;

pub struct HeimdallrLayer {
    pub(crate) registry_state: RegistryState,
    pub(crate) output_state: OutputState,
    pub(crate) shm: Shm,
    pub(crate) pool: Option<SlotPool>,
    pub(crate) layer: Option<LayerSurface>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) first_configure: bool,
    // pub(crate) input_region: Option<wl_region::WlRegion>,
    pub(crate) icons: HashMap<String, AlarmIcon>,
    pub(crate) battery_integrated: Option<crate::battery::BatteryStats>,
    pub(crate) needs_redraw: bool,
    pub(crate) last_redraw: Instant,
    pub(crate) redraw_interval: [Duration; 2],
    pub(crate) buffers: [Option<Buffer>; 2],
    pub(crate) current_buffer_idx: usize,
    pub(crate) config: crate::config::Config,
    pub(crate) notifications: Vec<crate::notifications::Notification>,
    pub(crate) notification_idx: usize,
    pub(crate) wob_value: f64,
    pub(crate) wob_expiration: Option<Instant>,
    pub(crate) ratatoskr_connected: bool,
    pub(crate) animator: Animator,
    pub(crate) frame_model: FrameModel,
    pub(crate) is_waiting_for_frame: bool,
    pub(crate) clock: crate::clock::ClockWrapper,
    pub(crate) security: crate::security::MicCameraStatus,
    pub(crate) last_security_width: f64,
    pub(crate) last_security_text: String,
    pub(crate) batteries: Vec<BatteryDevice>,
    pub(crate) last_batteries_width: f64,
    pub(crate) last_batteries_text: String,
    pub(crate) batteries_pristine: bool,
    pub(crate) timer: Countdown
}

impl HeimdallrLayer {
    pub fn check_redraw_timeout(&mut self) {

        if self.timer.is_active() && self.last_redraw.elapsed() > Duration::from_secs(1) {
            self.request_redraw("timer tick");
        } else if self.last_redraw.elapsed() > self.redraw_interval[1] {
            self.request_redraw("time");
        }
    }
    
    pub fn request_redraw(&mut self, _reason: &str) {
        self.needs_redraw = true;
        dbg_println!("{}", format!("Redraw requested by {}", _reason).yellow());
    }

    pub fn maybe_redraw(&mut self, qh: &QueueHandle<Self>) {

        // Now, updateNotificationList is for both adding new, and removing expired, notifications
        self.update_notification_list(None);

        // Check if wob-like must be closed
        if let Some(exp) = self.wob_expiration {
            if Instant::now() > exp {
                self.animator.animate_property(&self.frame_model, AnimationKey::WobHeightRatio, 0.0, 500);
                self.wob_expiration = None;
            }
        }

        let animating = self.animator.step(&mut self.frame_model);
        if !animating { // Now, we skip calling draw only if we are not animating something

            if !self.needs_redraw {
                return;
            }

            if self.last_redraw.elapsed() < self.redraw_interval[0] {
                return;
            }
        }

        self.needs_redraw = false;

        // qui fai il rendering vero e proprio:
        self.draw(qh);
    }

    fn acquire_buffer(buffers: &mut [Option<Buffer>; 2], width: u32, height: u32, current_buffer_idx: usize, pool: &mut SlotPool) -> Option<usize> {
        let stride = width as i32 * 4;
        let buffer_idx = current_buffer_idx;

        if buffers[buffer_idx].is_none() {
            let (new_buffer, _canvas) = pool
                .create_buffer(width as i32, height as i32, stride, wl_shm::Format::Argb8888)
                .expect("buffer creation failed");
            buffers[buffer_idx] = Some(new_buffer);
            dbg_println!("Buffer created");
        }
        if let Some(buffer) = buffers[buffer_idx].as_mut() {
            if buffer.canvas(pool).is_some() {
                return Some(buffer_idx);
            }
        }

        /*let other_idx = 1 - buffer_idx;
        if buffers[other_idx].is_none() {
            let (new_buffer, _canvas) = pool
                .create_buffer(width as i32, height as i32, stride, wl_shm::Format::Argb8888)
                .expect("buffer creation failed");
            buffers[other_idx] = Some(new_buffer);
        }
        if let Some(buffer) = buffers[other_idx].as_mut() {
            if buffer.canvas(pool).is_some() {
                return Some(other_idx);
            }
        }*/

        /* let (new_buffer, _canvas) = pool
            .create_buffer(width as i32, height as i32, stride, wl_shm::Format::Argb8888)
            .expect("buffer creation failed");
        buffers[buffer_idx] = Some(new_buffer);
        buffer_idx */
        None
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) {
        if self.is_waiting_for_frame {
            return;
        }
        if self.layer.is_some() && self.pool.is_some() {
            self.needs_redraw = false;
            let _start = std::time::Instant::now();

            let pool = self.pool.as_mut().unwrap();
            let buffer_idx_opt = Self::acquire_buffer(&mut self.buffers, self.width, self.height, self.current_buffer_idx, pool);
            if let Some(buffer_idx) = buffer_idx_opt {
                let buffer = self.buffers[buffer_idx].as_ref().unwrap();
                let canvas = buffer.canvas(pool).expect("canvas should be available immediately");
                let surface = unsafe {
                    ImageSurface::create_for_data_unsafe(
                        canvas.as_mut_ptr(),
                        Format::ARgb32,
                        self.width as i32,
                        self.height as i32,
                        buffer.stride(),
                    )
                    .unwrap()
                };

                self.update_timer_icon();

                let cr = Context::new(&surface).unwrap();
                self.check_batteries_data(&cr);
                self.check_security_data(&cr);

                self.draw_myframe(cr.clone());
                self.clock.draw(cr.clone(), self.height as i32, self.width, self.battery_integrated.clone());
                if self.notifications.len() > 0 { self.draw_notification(cr.clone()) }

                self.draw_batteries(cr.clone());
                self.draw_security(cr.clone());
                // self.draw_timer_2(&cr);
                self.draw_test_pill(&cr);

                let layer = self.layer.clone().unwrap();
                let buffer = self.buffers[buffer_idx].as_ref().unwrap();
                buffer.attach_to(layer.wl_surface()).unwrap();
                layer.wl_surface().damage_buffer(0, 0, self.width as i32, self.height as i32);
                // layer.wl_surface().damage_buffer(0, 0, self.width as i32, 50);
                // layer.wl_surface().damage_buffer(0, 0, 50, self.height as i32);
                self.is_waiting_for_frame = true;
                layer.wl_surface().frame(qh, layer.wl_surface().clone());
                layer.commit();

                drop(surface);
                // self.current_buffer_idx = 1 - buffer_idx;
                self.current_buffer_idx = (buffer_idx + 1) % self.buffers.len();
                self.last_redraw = Instant::now();

                #[cfg(debug_assertions)] {
                    let end = std::time::Instant::now();
                    let dur = (end - _start).as_nanos();
                    unsafe {
                        AVG_CNT += 1;
                        if AVG_CNT > -1 {
                            AVG_DUR += dur;
                            eprintln!("Draw ended ({:.2}ms avg {:.2}ms)", (dur as f64) / 1_000_000.0, ((AVG_DUR as f64)/(AVG_CNT as f64)) / 1_000_000.0); }
                        }
                }
            } else {
                dbg_println!("No available buffer to use");
            }
        }
    }

    fn build_batteries_text (&self) -> String {
        self.batteries.iter().map(|b| format!("{}: {:.0}%", b.name, b.percentage)).collect::<Vec<_>>().join("  ·  ")
    }

    fn check_batteries_data(&mut self, cr: &Context) {
        if self.batteries_pristine {
            self.batteries_pristine = false;
            let text = self.build_batteries_text();
            self.last_batteries_text = text;
            self.animator.animate_property(
                &self.frame_model,
                AnimationKey::BatteriesNotchRatio,
                if self.last_batteries_text.is_empty() { 0.0 } else { 1.0 },
                200
            );
            if self.last_batteries_text.is_empty() {
                return;
            }
            cr.set_font_size(10.0);
            cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
            if let Ok(ext) = cr.text_extents(&self.last_batteries_text) {
                self.last_batteries_width = ext.width() + 6.0;
            } else {
                self.last_batteries_width = 0.0;
            }
        }
    }
    
    fn build_security_text (&self) -> String {
        /* (
            self.security.mic_active.clone().into_iter().map(|s| format!("MIC {s}")).collect::<Vec<_>>().join("  ·  "), 
            self.security.camera_active.clone().into_iter().map(|s| format!("CAM {s}")).collect::<Vec<_>>().join("  ·  ")
        ) */
       self.security.mic_active.clone().into_iter().map(|s| format!("MIC {s}")).chain(self.security.camera_active.clone().into_iter().map(|s| format!("CAM {s}"))).collect::<Vec<_>>().join("  ·  ")
    }

    fn check_security_data(&mut self, cr: &Context) {
        if self.security.pristine {
            self.security.pristine = false;
            let text = self.build_security_text();
            self.last_security_text = text;
            self.animator.animate_property(
                &self.frame_model,
                AnimationKey::SecurityNotchRatio,
                if self.last_security_text.is_empty() { 0.0 } else { 1.0 },
                200
            );
            if self.last_security_text.is_empty() {
                return;
            }
            cr.set_font_size(10.0);
            cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
            if let Ok(ext) = cr.text_extents(&self.last_security_text) {
                self.last_security_width = ext.width() + 6.0;
            } else {
                self.last_security_width = 0.0;
            }
        }
    }

    fn draw_batteries (&mut self, cr: Context) {
        if self.last_batteries_text.len() > 0 {
            let color = (1.0, 1.0, 1.0, self.frame_model.batteries_height);
            

            cr.set_source_rgba(color.0, color.1, color.2, color.3);
            cr.set_font_size(10.0);
            cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
            // let x = 2.0;
            // let y = self.height as f64 / 2.0;
            // let (wt, ht) = cr_text_rotated(&cr, &self.last_batteries_text, x, y, 0.5, 0.0, -90.0).unwrap();
            let x = self.width as f64 - self.clock.get_reserved_width() - 4.0;
            let y = self.height as f64 - 1.0;
            cr_text_aligned(cr, self.last_batteries_text.clone(), x, y, 1.0, 1.0);
        }
    }

    fn draw_security (&mut self, cr: Context) {
        let draw_mic = self.security.mic_active.len() > 0;
        let draw_cam = self.security.camera_active.len() > 0;
        if draw_mic || draw_cam {
            let r = 4.0;
            let mic_color = (1.0, 0.58, 0.0, self.frame_model.security_height);
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
            rounded_rect_gradient(&cr, (self.width as f64 - self.last_security_width) / 2.0, 0.0, self.last_security_width, 12.0, r, steps, crate::utils::GradientDirection::Horizontal, false, None);

            cr.set_source_rgba(0.0, 0.0, 0.0 ,1.0);
            // cr.move_to(14.0, 9.0);
            // cr.show_text(&mic).unwrap();
            cr_text_aligned(cr.clone(), self.last_security_text.clone(), self.width as f64 / 2.0, 2.0, 0.5, 0.0);
            /* for app in self.security.mic_active.clone().into_iter() {
                cr.move_to(14.0, 9.0);
                cr.show_text(&app).unwrap();
                // let w = cr.text_extents(&text).unwrap().width();
                // cr_text_aligned(cr.clone(), app.into(), self.width / 2.0, 0.5, 0.0);
            } */
        }
    }

    fn draw_test_pill (&mut self, cr: &Context) {
        // I'm experimenting with new UI: some of this shit will be moved out of here, ofc!

        let mut pill_clock: PillClock = PillClock::new();
        pill_clock.update_data(&cr);
        let pill_clock_rect = pill_clock.get_desired_rect();

        let mut pill_battery = PillBattery::new();
        pill_battery.update_data(&cr, self.battery_integrated.clone());
        let pill_battery_rect = pill_battery.get_desired_rect();

        let mut pill_warnings = PillWarnings::new();
        let icons: Vec<AlarmIcon> = self.icons.values().cloned().filter(|icon| icon.symbol != "󱫡" && icon.symbol != "󱫌").collect();
        pill_warnings.update_data(&cr, icons);
        let pill_warnings_rect = pill_warnings.get_desired_rect();

        let c = Countdown {
            state: self.timer.state,
            total_paused_time: self.timer.total_paused_time,
            current_pause_start: self.timer.current_pause_start,
            direction: self.timer.direction.clone()
        }; /* Countdown::new();
        if self.timer.is_active() {
            let _ = c.fill_from_timespan(&*self.timer.format_custom_duration().1);
        } else {
            // let _ = c.fill_from_timespan("3m10s".into());
        } */
        let mut pill_countdown = PillCountdown::new(c);
        pill_countdown.update_data(&cr);
        let pill_countdown_rect = pill_countdown.get_desired_rect();

        let r = 8.0;
        let pill_bg_color: (f64, f64, f64, f64) = (0.1, 0.1, 0.15, 0.85);

        let space = 6.0;
        let rect_width = 
                space +
                pill_clock_rect.0 + space +
                pill_battery_rect.0 + space +
                if self.timer.is_active() { pill_countdown_rect.0 + space } else { 0.0 } + 
                if self.icons.len() > 0 { (self.icons.len() as f64) * 20.0 + space } else { 0.0 };
        let rect_height = 26.0;
        let rect_left = (self.width as f64 - rect_width) / 2.0;
        let rect_top = 2.0 + 24.0 * self.frame_model.notif_height_ratio;
        let mut x = rect_left + space;

        cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Bold);
        cr.set_font_size(16.0);

        /* let steps = vec![
            (0.0, (color.0, color.1, color.2, color.3)),
            (self.timer.progress(), (color.0, color.1, color.2, 0.5))
        ]; */
        // rounded_rect_gradient(&cr, rect_right - rect_width, rect_top, rect_width, rect_height, r, steps, crate::utils::GradientDirection::Horizontal, false, Some((0.0, 0.0, 0.0, 0.0)));
        rounded_rect_gradient(&cr, rect_left, rect_top, rect_width, rect_height, r, vec![(0.0, pill_bg_color)], crate::utils::GradientDirection::Horizontal, false, Some((0.0, 0.0, 0.0, 0.2)));

        pill_clock.draw(&cr, pill_clock_rect.0, rect_height, x, rect_top);
        x += pill_clock_rect.0 + space;

        pill_battery.draw(&cr, pill_battery_rect.0, rect_height, x, rect_top);
        x += pill_battery_rect.0 + space;

        if self.timer.is_active() {
            pill_countdown.draw(&cr, pill_countdown_rect.0, rect_height, x, rect_top);
            x += pill_countdown_rect.0 + space;
        }

        let mut switched = true;
        for icon in self.icons.values() {
            if &icon.symbol == "󱫡" || &icon.symbol == "󱫌" {
                continue;
            }
            if switched {
                cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);
                cr.set_font_size(16.0);
            }
            cr.set_source_rgba(icon.color.0, icon.color.1, icon.color.2, icon.color.3);
            cr.move_to(x, rect_top + rect_height / 2.0 + 3.0);
            cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);
            cr.show_text(&icon.symbol).unwrap();
            x += 20.0;
        }

/*
        let color = if passed { (1.0, 0.0, 0.3, 1.0) } else if self.timer.progress() < 0.9 { (1.0, 1.0, 1.0, 0.75) } else { (color.0, color.1, color.2, 0.75) };
        cr.set_source_rgba(color.0, color.1, color.2, color.3);
        cr_text_aligned(cr.clone(), time.clone(), rect_right, rect_top + rect_height / 2.0, 1.0, 0.5);
*/

        eprintln!("Test pill drawn");
    }

    fn draw_timer (&mut self, cr: &Context) {
        if !self.timer.is_active() {
            return;
        }
        let r = 5.0;
        let color = (1.0, 0.58, 0.0, 1.0);
        let rect_width = 100.0;
        let rect_height = 12.0;
        let rect_right = self.width as f64 - self.clock.get_reserved_width() - 16.0;
        let rect_top = 10.0;
        // let cam_color = (0.2, 0.78, 0.35, 1.0);
        /* let x = 1.0;
        let y = 1.0;
        let w = 10.0;
        let h = 10.0;
        let steps = if draw_mic && draw_cam { vec![(0.0, mic_color), (1.0, cam_color)] } else if draw_mic { vec![(0.0, mic_color)] } else { vec![(0.0, cam_color)] };
        rounded_rect_gradient(&cr, x, y, w, h, r, steps, crate::utils::GradientDirection::Horizontal, true, None); */

        cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Bold);
        cr.set_font_size(24.0);

        let steps = vec![
            (0.0, (color.0, color.1, color.2, color.3)),
            (self.timer.progress(), (color.0, color.1, color.2, 0.5))
        ];
        // rounded_rect_gradient(&cr, rect_right - rect_width, rect_top, rect_width, rect_height, r, steps, crate::utils::GradientDirection::Horizontal, false, Some((0.0, 0.0, 0.0, 0.0)));

        let (passed, mut time) = self.timer.format_custom_duration();
        if passed {
            time = format!("+{time}");
        }

        cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
        cr_text_aligned(cr.clone(), time.clone(), rect_right - 1.0, rect_top + rect_height / 2.0 - 1.0, 1.0, 0.5);

        cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
        cr_text_aligned(cr.clone(), time.clone(), rect_right + 1.0, rect_top + rect_height / 2.0 + 1.0, 1.0, 0.5);

        let color = if passed { (1.0, 0.0, 0.3, 1.0) } else if self.timer.progress() < 0.9 { (1.0, 1.0, 1.0, 0.75) } else { (color.0, color.1, color.2, 0.75) };
        cr.set_source_rgba(color.0, color.1, color.2, color.3);
        cr_text_aligned(cr.clone(), time.clone(), rect_right, rect_top + rect_height / 2.0, 1.0, 0.5);

        eprintln!("Timer {},{} (progress {})", passed, time, self.timer.progress());
    }

    fn draw_timer_2 (&mut self, cr: &Context) {
        if !self.timer.is_active() {
            return;
        }
        let r = 5.0;
        let color = (1.0, 0.58, 0.0, 1.0);
        let rect_width = 100.0;
        let rect_height = 12.0;
        let rect_right = self.width as f64 - self.clock.get_reserved_width() - 16.0;
        let rect_top = 6.0;
        // let cam_color = (0.2, 0.78, 0.35, 1.0);
        /* let x = 1.0;
        let y = 1.0;
        let w = 10.0;
        let h = 10.0;
        let steps = if draw_mic && draw_cam { vec![(0.0, mic_color), (1.0, cam_color)] } else if draw_mic { vec![(0.0, mic_color)] } else { vec![(0.0, cam_color)] };
        rounded_rect_gradient(&cr, x, y, w, h, r, steps, crate::utils::GradientDirection::Horizontal, true, None); */

        let (passed, mut time) = self.timer.format_custom_duration();
        if passed {
            time = format!("+{time}");
        }

        let color = if passed { (1.0, 0.0, 0.3, 1.0) } else if self.timer.progress() < 0.9 { (1.0, 1.0, 1.0, 0.75) } else { (color.0, color.1, color.2, 0.75) };
        // cr.set_source_rgba(color.0, color.1, color.2, color.3);

        cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(10.0);

        let steps = vec![
            (0.0, (color.0, color.1, color.2, color.3)),
            (self.timer.progress(), (color.0, color.1, color.2, color.3 * 0.5))
        ];
        rounded_rect_gradient(&cr, rect_right - rect_width, rect_top, rect_width, rect_height, r, steps, crate::utils::GradientDirection::Horizontal, false, Some((0.0, 0.0, 0.0, 0.0)));

        cr.set_source_rgba(0.0, 0.0, 0.0 ,1.0);
        // cr.move_to(14.0, 9.0);
        // cr.show_text(&mic).unwrap();
        // let (passed, time) = self.timer.format_custom_duration();
        cr_text_aligned(cr.clone(), time, rect_right - rect_width / 2.0, rect_top + rect_height / 2.0, 0.5, 0.5);
        /* for app in self.security.mic_active.clone().into_iter() {
            cr.move_to(14.0, 9.0);
            cr.show_text(&app).unwrap();
            // let w = cr.text_extents(&text).unwrap().width();
            // cr_text_aligned(cr.clone(), app.into(), self.width / 2.0, 0.5, 0.0);
        } */
    }

    fn update_timer_icon (&mut self) {
        // 󱫟 for pause
        // 󱫌 alert
        if self.timer.is_active() {
            let status = self.timer.format_custom_duration();
            let w = if status.0 { 1.0 } else { self.timer.get_warning() };
            let icon = if status.0 { "󱫌" } else { "󱫡" };
            self.add_icon("timer", icon, get_color_gradient(w), w, Some(status.1));
        } else {
            self.remove_icon("timer");
        }
    }

    fn draw_myframe(&mut self, cr: Context) {
        // cr.set_operator(cairo::Operator::Source);

        // Clear with full transparency
        // cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        cr.set_operator(cairo::Operator::Clear);
        cr.paint().unwrap();
        cr.set_operator(cairo::Operator::Over);

        // icons space reserved
        let mut y_offset = self.height as f64 - 8.0; // parte dal basso
        let res_w = 24.0;
        // let res_h = if self.ratatoskr_connected { (self.icons.len() as f64) * 30.0 } else { 30.0 };
        let res_h = self.frame_model.icons_ratio * 24.0;
        let wob_h = 24.0 * self.frame_model.wob_height;

        // Draw rounded rectangle frame
        let thickness = 1.0;
        let radius = 25.0;
        let radius2 = 4.0;

        let w = self.width as f64;
        let h = self.height as f64;
        let w_hole = w - thickness - self.clock.get_reserved_width() - 2.0;

        let top = thickness / 2.0 + /*if self.notifications.len() > 0 { 24.0 } else { 0.0 }*/24.0 * self.frame_model.notif_height_ratio;

        // Outer black border + fill
        // rounded_big_hole(&cr, thickness / 2.0, top, w_hole, h - thickness - top, radius, radius2, res_w, res_h, wob_h);

        let mut spaces = vec![
            // ReservedSpace { anchor: Anchor::BottomRight, width: 100.0, height: 40.0 }
            // ReservedSpace { anchor: Anchor::BottomLeft, width: res_w, height: res_h }
            // ReservedSpace { anchor: Anchor::TopRight, width: 90.0, height: 20.0 }
        ];
        // if let Some(bat) = &self.battery_integrated {
        if self.last_batteries_text.len() > 0 {
            spaces.push(ReservedSpace { anchor: Anchor::BottomRight, width: self.last_batteries_width, height: 14.0 });
        }
        if res_h > 0.0 {
            spaces.push(ReservedSpace { anchor: Anchor::BottomLeft, width: res_w, height: res_h });
        }
        if wob_h > 0.0 {
            spaces.push(ReservedSpace { anchor: Anchor::BottomCenter, width: 200.0, height: wob_h });
        }
        if self.frame_model.security_height > 0.0 /* && (self.security.mic_active.len() > 0 || self.security.camera_active.len() > 0) */ {
            cr.set_font_size(10.0);
            cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
            spaces.push(ReservedSpace { anchor: Anchor::TopCenter, width: self.last_security_width + 2.0, height: 14.0 * self.frame_model.security_height });
        }
        draw_smart_border(&cr, thickness / 2.0, top, w_hole, h - thickness/2.0 - top, w / 2.0, h / 2.0, radius, radius2, &&spaces);

        cr.set_fill_rule(cairo::FillRule::EvenOdd);
        cr.rectangle(-1.0, -1.0, w + 2.0, h + 2.0);

        cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
        

        if let Some((r, g, b, a)) = match self.config.frame_color {
            FrameColor::Rgba(r, g, b, a) => Some((r, g, b, a)),
            FrameColor::WorstResource => self
                .icons
                .values()
                .max_by(|a, b| a.warn.partial_cmp(&b.warn).unwrap_or(std::cmp::Ordering::Equal))
                .map(|icon| icon.color),
            FrameColor::None /* | FrameColor::Random */ => None,
        } {
            cr.fill_preserve().unwrap();
            cr.set_line_width(1.0);
            cr.set_source_rgba(r, g, b, a);
            // rounded_big_hole(&cr, thickness / 2.0 + 1.0, top, w_hole, h - thickness - top, radius, radius2, res_w, res_h, wob_h);
            cr.stroke().unwrap();
        } else {
            cr.fill().unwrap();
        }

        // wob-like
        let mut steps = vec![(0.0, (1.0, 1.0, 1.0, self.frame_model.wob_height))];
        let xc = (self.width as f64) / 2.0;
        if wob_h > 2.0 {
            match self.config.frame_color {
                FrameColor::None => {
                    steps.push((self.wob_value, (1.0, 1.0, 1.0, 0.3)));
                },
                _ => {
                    steps.push((self.wob_value, (0.0, 0.0, 0.0, 0.0)));
                }
            }

            let wob_half_width = 98.0;
            let wob_height = wob_h - 4.0;
            if wob_height > 0.0 {
                let left = xc - wob_half_width;
                let top = h - thickness - (thickness / 2.0) - wob_h + 3.0;
                rounded_rect_gradient(&cr, left, top, wob_half_width * 2.0, wob_height, wob_h.min(radius2-1.0), steps, crate::utils::GradientDirection::Horizontal, false, None);
            }
        }


        // Draw battery level if integrated battery is present

        /* if let Some(bat) = &self.battery_integrated {
            let x = self.width as f64 - 2.0;
            let y = self.height as f64 - 0.0;
            cr.set_source_rgba(1.0, 1.0, 1.0 ,1.0);
            cr.set_font_size(9.0);

            cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
            let (wt, ht) = cr_text_rotated(&cr, &*format!("{:.0}%", bat.percentage), x, y - 2.0, 1.0, 0.0, 90.0).unwrap();

            cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);
            let _ = cr_text_rotated(&cr, "󰌢", x, y - wt - 2.0, 1.0,0.0, 90.0);
        } */

        /* let mut x = self.width as f64 - self.clock.get_reserved_width() - 2.0 - 42.0;
        let mut y = 2.0; // self.height as f64 - 2.0
        if let Some(bat) = &self.battery_integrated {
            // spaces.push(ReservedSpace { anchor: Anchor::BottomRight, width: 60.0, height: 20.0 });
            if bat.state == crate::battery::BatteryState::Charging {
                cr.set_source_rgba(0.1, 1.0, 0.2, 1.0);
            } else if bat.state == crate::battery::BatteryState::Discharging {
                cr.set_source_rgba(1.0, 0.1, 0.2, 1.0);
            } else {
                cr.set_source_rgba(1.0, 1.0, 1.0 ,1.0);
            }
            cr.set_font_size(10.0);

            cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);
            cr_text_aligned(cr.clone(), "󰌢".to_string(), x, y, 0.0, 0.0);

            cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
            cr_text_aligned(cr.clone(), format!("{:.0}%", bat.percentage), x + 14.0, y, 0.0, 0.0);
        }

        y += 14.0;
        cr.set_source_rgba(1.0, 1.0, 1.0 ,1.0);
        cr.set_font_size(10.0);

        cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);
        cr_text_aligned(cr.clone(), "󰦋".to_string(), x, y, 0.0, 0.0);

        cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
        cr_text_aligned(cr.clone(), format!("{:.0}%", 90.0), x + 14.0, y, 0.0, 0.0);
        */

        // === Draw alarm icons ===
        
        let mut switched = true;
        for icon in self.icons.values() {
            if switched {
                cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);
                cr.set_font_size(16.0);
            }
            cr.set_source_rgba(icon.color.0, icon.color.1, icon.color.2, icon.color.3);
            cr.move_to(4.0, y_offset);
            cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);
            cr.show_text(&icon.symbol).unwrap();
            if let Some(info) = &icon.info {
                switched = true;
                cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
                cr.set_font_size(12.0);

                /* cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
                cr.move_to(24.0 - 1.0, y_offset - 1.0);
                cr.show_text(&info).unwrap();

                cr.set_source_rgba(icon.color.0, icon.color.1, icon.color.2, icon.color.3);
                cr.move_to(24.0, y_offset);
                cr.show_text(&info).unwrap(); */

                cr.move_to(24.0, y_offset);
                cr.text_path(&info);
    
                let path = cr.copy_path().expect("Valid path");

                // border (stroke)
                cr.set_source_rgb(0.0, 0.0, 0.0);
                cr.set_line_width(2.0);
                cr.set_line_join(cairo::LineJoin::Round);
                cr.stroke().expect("Stroke failed");

                // text (fill)
                cr.append_path(&path);
                cr.set_source_rgba(icon.color.0, icon.color.1, icon.color.2, icon.color.3);
                cr.fill().expect("Fill failed");


            } else {
                switched = false;
            }
            y_offset -= 24.0;
        }

    }

    fn draw_notification(&mut self, cr: Context) {
        if self.notification_idx >= self.notifications.len() {
            self.notification_idx = self.notifications.len() - 1;
        }
        // icon example: /home/vncnz/.cache/ignis/notifications/images/64
        cr.set_operator(cairo::Operator::Over);

        // let top = thickness / 2.0 + if self.notifications.len() > 0 { 24.0 } else { 0.0 };
        let top = 12.0;
        let notif_to_show = &self.notifications[0];

        cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Bold);

        let mut x = 25.0;

        cr.set_font_size(16.0);
        if let FrameColor::Rgba(r,g,b,a) = self.config.frame_color {
            cr.set_source_rgba(r,g,b,if self.notifications.len() > 1 { a } else { a/2.0 } );
        } else {
            cr.set_source_rgba(1.0,1.0,1.0,if self.notifications.len() > 1 { 1.0 } else { 0.5 } );
        }
        let idx = format!("{}/{}", self.notification_idx+1, self.notifications.len());
        let (idx_width, _) = cr_text_aligned(cr.clone(), idx, x, top, 0.0, 0.5);
        x += idx_width + 10.0;

        cr.set_font_size(16.0);
        if notif_to_show.urgency == 2 {
            cr.set_source_rgba(1.0, 0.3, 0.3, 1.0);
        } else {
            cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        }
        let (twidth, _) = cr_text_aligned(cr.clone(), notif_to_show.app_name.clone(), x, top, 0.0, 0.5);
        x += twidth + 10.0;

        cr.set_font_size(14.0);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        let msg = if notif_to_show.body.is_empty() {
            notif_to_show.summary.clone()
        } else {
            format!("{} / {}", notif_to_show.summary, notif_to_show.body)
        };
        cr_text_aligned(cr.clone(), msg, x, top, 0.0, 0.5);
    }

    pub fn update_notification_list (&mut self, new_notif_opt: Option<Notification>) {

        let mut changed: bool = false;
        if let Some(new_notif) = new_notif_opt {
            let mut custom_replace = None;
            if new_notif.unmounted {
                let to_be_replaced = self.notifications.iter().find(|x| x.unmounting);
                if let Some(notif) = to_be_replaced {
                    custom_replace = Some(notif.id);
                }
            }
            if let Some(rep) = custom_replace {
                self.notifications.retain(|n| n.id != rep);
            }

            if new_notif.replaces_id > 0 {
                self.notifications.retain(|n| n.id != new_notif.replaces_id);
            }

            // let id = list.iter().map(|x| x.id).max().unwrap_or();
            
            self.notifications.insert(0, new_notif);
            changed = true;
        }

        let a = self.notifications.len();
        self.notifications.retain(|n| n.expired_at.is_none() || (n.expired_at.unwrap() > Instant::now()));
        let b = self.notifications.len();

        changed = changed || (a != b);

        if changed {
            self.animator.animate_property(
                &self.frame_model,
                AnimationKey::NotificationHeight,
                if self.notifications.len() > 0 { 1.0 } else { 0.0 },
                200
            );
            self.request_redraw("notifications updated");
        }

    }
}

impl HeimdallrLayer { // This is for icon/notifications/stuff management, I like to keep it separated
    pub fn add_icon(&mut self, id: &str, symbol: &str, color: (f64, f64, f64, f64), warn: f64, info: Option<String>) -> IconChange {

        let mut already_present = false;
        if let Some(found) = self.icons.get(id) {
            already_present = true;
            if f64::abs(found.warn - warn) < 0.05 && found.info == info {
                return IconChange::None;
            }
        }

        self.icons.insert(
            id.to_string(),
            AlarmIcon {
                symbol: symbol.to_string(),
                color,
                warn,
                info
            },
        );
        if already_present {
            IconChange::Changed
        } else {
            self.animator.animate_property(
                &self.frame_model,
                AnimationKey::IconsHeight,
                self.icons.len() as f64,
                200
            );
            IconChange::Added
        }
    }

    pub fn remove_icon(&mut self, id: &str) -> bool {
        let removed = self.icons.remove(id).is_some();
        if removed {
            self.animator.animate_property(
                &self.frame_model,
                AnimationKey::IconsHeight,
                self.icons.len() as f64,
                200
            );
        }
        removed
    }

    pub fn remove_notification(&mut self) -> bool {
        if self.notifications.len() > self.notification_idx {
            self.notifications.remove(self.notification_idx);
            if self.notification_idx > self.notifications.len() { self.notification_idx = 0 }
            return true
        }
        return false
    }
    
    pub fn show_notification(&mut self, new_idx: i32) -> bool {
        if new_idx >= 0 && new_idx < self.notifications.len() as i32 {
           self.notification_idx = new_idx as usize;
           return true
        }
        false
    }

    pub fn show_value(&mut self, value: f64, _kind: Option<&str>) -> bool {
        let changed = self.wob_expiration.is_none() || self.wob_value != value;
        self.wob_expiration = Some(Instant::now() + Duration::from_millis(2000));
        self.wob_value = value.clamp(0.0, 1.0);
        changed
    }
}

/* fn wob_rect (cr: &Context, xc: f64, yb: f64, r2: f64, wob_h: f64, wob_value: f64) {
    if wob_h > 0.0 {
        cr.new_sub_path();

        let r2_safe = if wob_h > r2 { r2 } else { wob_h/2.0 };
        let wob_half_width = 100.0;
        cr.arc(xc + r2_safe + (wob_half_width * 2.0 * (wob_value - 0.5)), yb - r2_safe, r2_safe, 90f64.to_radians(), 180f64.to_radians());
        cr.arc_negative(xc - r2_safe + (wob_half_width * 2.0 * (wob_value - 0.5)), yb + r2_safe - wob_h, r2_safe, 0f64.to_radians(), 270f64.to_radians());
        cr.arc_negative(xc + r2_safe - wob_half_width, yb + r2_safe - wob_h, r2_safe, 270f64.to_radians(), 180f64.to_radians());
        cr.arc(xc - r2_safe - wob_half_width, yb - r2_safe, r2_safe, 0f64.to_radians(), 90f64.to_radians());

        cr.close_path();
    }
} */

impl CompositorHandler for HeimdallrLayer {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wayland_client::protocol::wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wayland_client::protocol::wl_surface::WlSurface, _: wayland_client::protocol::wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wayland_client::protocol::wl_surface::WlSurface, _: u32) {
        dbg_println!("SCTK Frame callback received");
        self.is_waiting_for_frame = false;
        self.maybe_redraw(qh);
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wayland_client::protocol::wl_surface::WlSurface, _: &wayland_client::protocol::wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wayland_client::protocol::wl_surface::WlSurface, _: &wayland_client::protocol::wl_output::WlOutput) {}
}

impl OutputHandler for HeimdallrLayer {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wayland_client::protocol::wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wayland_client::protocol::wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wayland_client::protocol::wl_output::WlOutput) {}
}

impl LayerShellHandler for HeimdallrLayer {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        eprintln!("LayerShell surface closed by compositor");
        log_to_file("LayerShell surface closed by compositor".to_string());
        std::process::exit(0);
    }

    fn configure(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &LayerSurface, configure: LayerSurfaceConfigure, _: u32) {
        eprintln!("LayerShell surface configured by compositor {:?}", configure.new_size);
        self.width = NonZeroU32::new(configure.new_size.0).map_or(1920, NonZeroU32::get);
        self.height = NonZeroU32::new(configure.new_size.1).map_or(1080, NonZeroU32::get);
        self.pool = Some(SlotPool::new((self.width * self.height * 4) as usize, &self.shm).expect("pool creation failed"));
        self.buffers = [None, None];
        self.current_buffer_idx = 0;
        if self.first_configure {
            self.first_configure = false;
            self.draw(qh);
        }
    }
}

impl ShmHandler for HeimdallrLayer {
    fn shm_state(&mut self) -> &mut Shm { &mut self.shm }
}

delegate_compositor!(HeimdallrLayer);
delegate_output!(HeimdallrLayer);
delegate_shm!(HeimdallrLayer);
delegate_layer!(HeimdallrLayer);
delegate_registry!(HeimdallrLayer);

impl ProvidesRegistryState for HeimdallrLayer {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry_state }
    registry_handlers![OutputState];
}

impl Dispatch<wl_compositor::WlCompositor, ()> for HeimdallrLayer {
    fn event(
        _state: &mut Self,
        _proxy: &wl_compositor::WlCompositor,
        _event: wl_compositor::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
        dbg_println!("Dispatch wlcompositor called");
    }
}

impl Dispatch<wl_region::WlRegion, ()> for HeimdallrLayer {
    fn event(
        _state: &mut Self,
        _proxy: &wl_region::WlRegion,
        _event: wl_region::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
        dbg_println!("Dispatch wlregion called");
    }
}