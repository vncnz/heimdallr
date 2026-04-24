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

use crate::{clock::ClockTrait, clock1::Clock1, config::FrameColor, dbg_println, notifications::Notification, utils::{AnimationKey, Animator, FrameModel, cr_text_aligned, get_color_gradient, log_to_file}};

#[derive(PartialEq)]
pub enum IconChange {
    Added,
    Changed,
    // Removed,
    None
}
pub struct AlarmIcon {
    symbol: String,
    color: (f64, f64, f64, f64), // RGBA
    warn: f64
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
    pub(crate) battery_eta: Option<f64>,
    pub(crate) battery_recharging: Option<bool>,
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
    pub(crate) clock: Box<dyn ClockTrait>
}

impl HeimdallrLayer {
    pub fn check_redraw_timeout(&mut self) {

        if self.last_redraw.elapsed() < self.redraw_interval[1] {
            return;
        }

        self.request_redraw("time");
    }
    
    pub fn request_redraw(&mut self, _reason: &str) {
        self.needs_redraw = true;
        dbg_println!("Redraw requested by {}", _reason);
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
                let cr = Context::new(&surface).unwrap();

                self.draw_myframe(cr.clone());
                self.clock.draw(cr.clone(), self.height as i32, self.width, self.battery_recharging, self.battery_eta);
                if self.notifications.len() > 0 { self.draw_notification(cr.clone()) }

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
        let res_h = if self.ratatoskr_connected { self.frame_model.icons_ratio * 24.0 } else { 24.0 };
        let wob_h = 24.0 * self.frame_model.wob_height;

        // Draw rounded rectangle frame
        let thickness = 1.0;
        let radius = 25.0;
        let radius2 = 4.0;

        let w = self.width as f64;
        let h = self.height as f64;
        let w_hole = w - thickness - self.clock.get_reserved_width();

        let top = thickness / 2.0 + /*if self.notifications.len() > 0 { 24.0 } else { 0.0 }*/24.0 * self.frame_model.notif_height_ratio;

        // Outer black border
        cr.rectangle(0.0, 0.0, w, h);
        cr.set_fill_rule(cairo::FillRule::EvenOdd);
        rounded_rect(&cr, thickness / 2.0, top, w_hole, h - thickness - top, radius, radius2, res_w, res_h, wob_h);
        cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
        cr.fill().unwrap();

        // wob-like
        let xc = (thickness + w_hole) / 2.0;
        if wob_h != 0.0 {
            match self.config.frame_color {
                FrameColor::None => {
                    wob_rect(&cr, xc, h - thickness, radius2, wob_h, 1.0);
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.35);
                    cr.fill().unwrap();

                    wob_rect(&cr, xc, h - thickness, radius2, wob_h, 1.0);
                    cr.set_source_rgba(0.0, 0.0, 0.0, 0.5);
                    cr.stroke().unwrap();
                },
                _ => {}
            }
        }

        wob_rect(&cr, xc, h - thickness, radius2, wob_h, self.wob_value);
        cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        cr.fill().unwrap();

        // === Draw alarm icons ===
        
        cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(16.0);
        if self.ratatoskr_connected {
            for icon in self.icons.values() {
                cr.set_source_rgba(icon.color.0, icon.color.1, icon.color.2, icon.color.3);
                cr.move_to(4.0, y_offset);
                cr.show_text(&icon.symbol).unwrap();
                y_offset -= 24.0;
            }
        } else {
            let red = get_color_gradient(1.0);
            cr.set_source_rgba(red.0, red.1, red.2, red.3);
            // cr.move_to(4.0, y_offset);
            cr_text_aligned(cr.clone(), "󰠗".to_string(), 12.0, y_offset, 0.5, 0.5);
        }

        // === Draw colored border ===
        if let Some((r, g, b, a)) = match self.config.frame_color {
            FrameColor::Rgba(r, g, b, a) => Some((r, g, b, a)),
            FrameColor::WorstResource => self
                .icons
                .values()
                .max_by(|a, b| a.warn.partial_cmp(&b.warn).unwrap_or(std::cmp::Ordering::Equal))
                .map(|icon| icon.color),
            FrameColor::None /* | FrameColor::Random */ => None,
        } {
            cr.set_line_width(1.0);
            cr.set_source_rgba(r, g, b, a);
            rounded_rect(&cr, thickness / 2.0 + 1.0, top, w_hole, h - thickness - top, radius, radius2, res_w, res_h, wob_h);
            cr.stroke().unwrap();
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

impl HeimdallrLayer { // This is for icon management, I like to keep it separated, for now
    pub fn add_icon(&mut self, id: &str, symbol: &str, color: (f64, f64, f64, f64), warn: f64) -> IconChange {

        let mut already_present = false;
        if let Some(found) = self.icons.get(id) {
            already_present = true;
            if f64::abs(found.warn - warn) < 0.05 {
                return IconChange::None;
            }
        }

        self.icons.insert(
            id.to_string(),
            AlarmIcon {
                symbol: symbol.to_string(),
                color,
                warn
            },
        );
        if already_present {
            IconChange::Changed
        } else {
            IconChange::Added
        }
    }

    pub fn remove_icon(&mut self, id: &str) -> bool {
        self.icons.remove(id).is_some()
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

fn wob_rect (cr: &Context, xc: f64, yb: f64, r2: f64, wob_h: f64, wob_value: f64) {
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
}

fn rounded_rect(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64, r2: f64, reserved_w: f64, reserved_h: f64, wob_h: f64) {
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -90f64.to_radians(), 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, 90f64.to_radians());

    if wob_h > 0.0 {
        let r2_safe = if wob_h > r2 { r2 } else { wob_h/2.0 };
        let wob_half_width = 100.0;
        cr.arc(x + w/2.0 + r2_safe + wob_half_width, y + h - r2_safe, r2_safe, 90f64.to_radians(), 180f64.to_radians());
        cr.arc_negative(x + w/2.0 - r2_safe + wob_half_width, y + h + r2_safe - wob_h, r2_safe, 0f64.to_radians(), 270f64.to_radians());
        cr.arc_negative(x + w/2.0 + r2_safe - wob_half_width, y + h + r2_safe - wob_h, r2_safe, 270f64.to_radians(), 180f64.to_radians());
        cr.arc(x + w/2.0 - r2_safe - wob_half_width, y + h - r2_safe, r2_safe, 0f64.to_radians(), 90f64.to_radians());
    }
    
    if reserved_h > 0.0 {
        let r2_safe = if reserved_h > r2 { r2 } else { reserved_h/2.0 };
        // dbg_println!("reserved_h: {}", reserved_h);
        cr.arc(x + r2_safe + reserved_w, y + h - r2_safe, r2_safe, 90f64.to_radians(), 180f64.to_radians());
        cr.arc_negative(x - r2_safe + reserved_w, y + h + r2_safe - reserved_h, r2_safe, 0f64.to_radians(), 270f64.to_radians());
        cr.arc(x + r2, y + h - r2 - reserved_h, r2, 90f64.to_radians(), 180f64.to_radians());
    } else {
        cr.arc(x + r, y + h - r, r, 90f64.to_radians(), 180f64.to_radians());
    }
    
    cr.arc(x + r, y + r, r, 180f64.to_radians(), 270f64.to_radians());
    cr.close_path();
}

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