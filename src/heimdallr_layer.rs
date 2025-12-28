use smithay_client_toolkit::{
    compositor::CompositorHandler, delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm, output::{OutputHandler, OutputState}, registry::{ProvidesRegistryState, RegistryState}, registry_handlers, shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure}, shm::{Shm, ShmHandler, slot::SlotPool}
};
use wayland_client::{Connection, Proxy, QueueHandle, backend::ObjectId, protocol::{wl_buffer::WlBuffer, wl_compositor, wl_region, wl_shm}};
use cairo::{Context, Format, ImageSurface};

use std::{num::NonZeroU32, time::{Duration, Instant}};

use smithay_client_toolkit::shell::WaylandSurface;

use std::collections::HashMap;
use cairo::FontSlant;

use wayland_client::protocol::wl_buffer;
use wayland_client::Dispatch;

use chrono::Local;
use chrono::Timelike;

use crate::{config::FrameColor, utils::{AnimationKey, Animator, FrameModel, get_color_gradient}};

pub struct AlarmIcon {
    symbol: String,
    color: (f64, f64, f64, f64), // RGBA
    warn: f64
}

fn cr_text_aligned (cr: Context, text: String, x: f64, y: f64, dx: f64, dy: f64) -> (f64, f64) {
    // if v != 0.0 || h != 0.0 {
        let mut x1 = x;
        let mut y1 = y;
        let extents = cr.text_extents(&text).unwrap();
        x1 -= extents.width() * dx;
        y1 -= extents.height() * dy + extents.y_bearing();
        cr.move_to(x1, y1);
        // eprintln!("({},{}) -> ({},{})   {:?}", &x, &y, &x1, &y1, extents);
    // }
    cr.show_text(&text).ok();
    (extents.width(), extents.height())
}

pub struct HeimdallrLayer {
    pub(crate) registry_state: RegistryState,
    pub(crate) output_state: OutputState,
    pub(crate) shm: Shm,
    pub(crate) pool: SlotPool,
    pub(crate) layer: LayerSurface,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) first_configure: bool,
    pub(crate) input_region: Option<wl_region::WlRegion>,
    pub(crate) icons: HashMap<String, AlarmIcon>,
    pub(crate) battery_eta: Option<f64>,
    pub(crate) battery_recharging: Option<bool>,
    pub(crate) needs_redraw: bool,
    pub(crate) last_redraw: Instant,
    pub(crate) redraw_interval: Duration,
    pub(crate) buffers: HashMap<ObjectId, wl_buffer::WlBuffer>,
    pub(crate) background_surface: Option<cairo::ImageSurface>,
    pub(crate) config: crate::config::Config,
    pub(crate) notifications: Vec<crate::notifications::Notification>,
    pub(crate) notification_idx: usize,
    pub(crate) ratatoskr_connected: bool,
    pub(crate) animator: Animator,
    pub(crate) frame_model: FrameModel
}

impl HeimdallrLayer {
    pub fn request_redraw(&mut self, reason: &str) {
        self.needs_redraw = true;
        #[cfg(debug_assertions)] {
            println!("Redraw requested by {}", reason);
        }
    }

    pub fn maybe_redraw(&mut self, qh: &QueueHandle<Self>) {
        let animating = self.animator.step(&mut self.frame_model);
        if !animating { // Now, we skip calling draw only if we are not animating something

            if !self.needs_redraw {
                return;
            }

            if self.last_redraw.elapsed() < self.redraw_interval {
                return;
            }
        }

        self.needs_redraw = false;

        // qui fai il rendering vero e proprio:
        self.draw(qh);
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) {
        self.needs_redraw = false;
        let start = std::time::Instant::now();
        // eprintln!("Draw started at {:?}", start);
        
        let stride = self.width as i32 * 4;
        let (buffer, canvas) = self
            .pool
            .create_buffer(self.width as i32, self.height as i32, stride, wl_shm::Format::Argb8888)
            .expect("buffer creation failed");

        let wl_buffer: WlBuffer = buffer.wl_buffer().clone();
        let id: ObjectId = wl_buffer.id();
        self.buffers.insert(id, wl_buffer);

        // Cairo surface on the shared memory buffer
        let surface = unsafe {
            ImageSurface::create_for_data_unsafe(
                canvas.as_mut_ptr(),
                Format::ARgb32,
                self.width as i32,
                self.height as i32,
                stride,
            )
            .unwrap()
        };
        let cr = Context::new(&surface).unwrap();

        self.draw_myframe(cr.clone());
        if self.config.show_clock { self.draw_clock(cr.clone()); }
        if self.notifications.len() > 0 { self.draw_notification(cr.clone()) }

        // Damage + commit
        buffer.attach_to(self.layer.wl_surface()).unwrap();
        self.layer.attach(Some(&buffer.wl_buffer()), 0, 0);
        self.layer.wl_surface().damage_buffer(0, 0, self.width as i32, self.height as i32);
        self.layer.wl_surface().frame(qh, self.layer.wl_surface().clone());
        self.layer.commit();

        drop(surface);
        
        self.last_redraw = Instant::now();

        #[cfg(debug_assertions)] {
            let end = std::time::Instant::now();
            let dur = (end - start).as_millis();
            eprintln!("Draw ended ({}ms)", dur);
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
        let mut y_offset = self.height as f64 - 2.0 - 8.0; // parte dal basso
        let res_w = 24.0;
        // let res_h = if self.ratatoskr_connected { (self.icons.len() as f64) * 30.0 } else { 30.0 };
        let res_h = if self.ratatoskr_connected { self.frame_model.icons_ratio * 20.0 + 6.0 + 8.0 } else { 24.0 };

        // Draw rounded rectangle frame
        let thickness = 1.0;
        let radius = 25.0;
        let radius2 = 4.0;

        let w = self.width as f64;
        let h = self.height as f64;
        let w_hole = w - thickness - (if self.config.show_clock { 8.0 } else { 0.0 });

        let top = thickness / 2.0 + /*if self.notifications.len() > 0 { 24.0 } else { 0.0 }*/24.0 * self.frame_model.notif_height_ratio;

        // Outer black border (semi-transparent)
        cr.rectangle(0.0, 0.0, w, h);
        cr.set_fill_rule(cairo::FillRule::EvenOdd);
        rounded_rect(&cr, thickness / 2.0, top, w_hole, h - thickness - top, radius, radius2, res_w, res_h);
        cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
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
            FrameColor::None | FrameColor::Random => None,
        } {
            cr.set_line_width(1.0);
            cr.set_source_rgba(r, g, b, a);
            rounded_rect(&cr, thickness / 2.0, top, w_hole, h - thickness - top, radius, radius2, res_w, res_h);
            cr.stroke().unwrap();
        }

            

    }

    fn draw_clock (&mut self, cr: Context) {
        if self.background_surface.is_none() {
            self.draw_clock_background();
        }
        if let Some(bg) = &self.background_surface {
            cr.set_source_surface(bg, (self.width - 18) as f64, 0.0).unwrap();
            cr.paint().unwrap();
        }

        let now = Local::now();
        let seconds_today =
        now.num_seconds_from_midnight() as f64 + f64::from(now.nanosecond()) / 1_000_000_000.0;
        let y = seconds_today / 86_400.0;
        let ypos = (1.0 - y) * (self.height as f64);
        // eprintln!("{} {}", y, ypos);

        /* Border */
        cr.set_source_rgba(0.1, 0.1, 0.1, 1.0);
        cr.move_to((self.width - 24u32) as f64 + 1.0, (1.0 - y) * (self.height as f64) - 1.0);
        cr.set_font_size(17.0);
        // cr.show_text("");
        cr_text_aligned(cr.clone(), "".into(), self.width as f64 - 5.0, ypos, 1.0, 0.0);
        /* end */

        cr.set_source_rgba(1.0, 0.1, 0.2, 1.0);
        cr.move_to((self.width - 24u32) as f64, (1.0 - y) * (self.height as f64));
        cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(15.0);
        cr_text_aligned(cr.clone(), "".into(), self.width as f64 - 5.0, ypos, 1.0, 0.0);

        if let Some(rec) = self.battery_recharging {
            // eprintln!("Battery moving");
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
            if let Some(eta) = self.battery_eta {
                let bpos = (ypos - (eta / 1440.0 * self.height as f64) + self.height as f64) % self.height as f64;
                
                /* Border */
                cr.set_font_size(font_size + 2.0);
                cr.set_source_rgba(0.1,0.1,0.1,1.0);
                cr_text_aligned(cr.clone(), bat_symb.clone(), self.width as f64 - 7.0 + 1.0, bpos, 1.0, 0.5);
                /* end */
                
                cr.set_font_size(font_size);
                let (r,g,b,a) = color;
                cr.set_source_rgba(r,g,b,a);
                cr_text_aligned(cr.clone(), bat_symb, self.width as f64 - 7.0, bpos, 1.0, 0.5);
            } else {
                cr_text_aligned(cr.clone(), bat_symb, self.width as f64, 0.0, 1.0, 0.5);
            }
            
            // let extents = cr.text_extents(text).unwrap();
        } else {
            // eprintln!("No battery info/moving");
        }
    }

    fn draw_clock_background(&mut self) {
        let width = 18;
        let height = self.height as i32;
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
}

impl HeimdallrLayer { // This is for icon management, I like to keep it separated, for now
    pub fn add_icon(&mut self, id: &str, symbol: &str, color: (f64, f64, f64, f64), warn: f64) {
        self.icons.insert(
            id.to_string(),
            AlarmIcon {
                symbol: symbol.to_string(),
                color,
                warn
            },
        );
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
}

fn rounded_rect(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64, r2: f64, reserved_w: f64, reserved_h: f64) {
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -90f64.to_radians(), 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, 90f64.to_radians());
    
    if reserved_h > 0.0 {
        let r2_safe = if reserved_h > r2 { r2 } else { reserved_h/2.0 };
        // eprintln!("reserved_h: {}", reserved_h);
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
    fn frame(&mut self, _: &Connection, _qh: &QueueHandle<Self>, _: &wayland_client::protocol::wl_surface::WlSurface, _: u32) {
        // self.draw(qh);
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
        std::process::exit(0);
    }

    fn configure(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &LayerSurface, configure: LayerSurfaceConfigure, _: u32) {
        self.width = NonZeroU32::new(configure.new_size.0).map_or(1920, NonZeroU32::get);
        self.height = NonZeroU32::new(configure.new_size.1).map_or(1080, NonZeroU32::get);
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

// Dispatch per ricevere wl_buffer::Event::Release
impl Dispatch<wl_buffer::WlBuffer, ()> for HeimdallrLayer {
    fn event(
        state: &mut Self,
        proxy: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        eprintln!("Dispatch wlbuffer called");
        match event {
            wl_buffer::Event::Release => {
                // ottieni l'id del proxy rilasciato
                let id = proxy.id();
                // rimuovi dalla mappa: l'ultimo WlBuffer viene droppato -> memoria liberata
                let removed = state.buffers.remove(&id);
                if removed.is_none() {
                    eprintln!("Release per buffer non presente in mappa: {}", id);
                } else {
                    eprintln!("Buffer {} rilasciato e rimosso", id);
                }
            }
            _ => {}
        }
    }
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
        eprintln!("Dispatch wlcompositor called");
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
        eprintln!("Dispatch wlregion called");
    }
}