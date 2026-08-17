use smithay_client_toolkit::{
    compositor::CompositorHandler, delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm, output::{OutputHandler, OutputState}, registry::{ProvidesRegistryState, RegistryState}, registry_handlers, shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure}, shm::{Shm, ShmHandler, slot::{Buffer, SlotPool}}
};
use wayland_client::{Connection, QueueHandle, protocol::{wl_compositor, wl_region, wl_shm}};
use cairo::{Context, Format, ImageSurface};

use std::{num::NonZeroU32, time::{Duration, Instant}};

use smithay_client_toolkit::shell::WaylandSurface;

use std::collections::HashMap;

use wayland_client::Dispatch;
use colored::Colorize;

use crate::{config::{Config, FrameColor}, data::{AlarmIcon, BatteryDevice, IconChange}, dbg_println, notifications::Notification, pills::{Pill, PillModuleTrait}, security::MicCameraStatus, utils::{TweenState, draw_smart_border, log_to_file, mix_color, rounded_rect_gradient}};

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
    // pub(crate) battery_integrated: Option<crate::battery::BatteryStats>,
    pub(crate) needs_redraw: bool,
    pub(crate) last_redraw: Instant,
    pub(crate) redraw_interval: [Duration; 2],
    pub(crate) buffers: [Option<Buffer>; 2],
    pub(crate) current_buffer_idx: usize,
    pub(crate) config: crate::config::Config,
    pub(crate) notifications: Vec<crate::notifications::Notification>,
    pub(crate) wob_value: TweenState,
    pub(crate) wob_expiration: Option<Instant>,
    pub(crate) ratatoskr_connected: bool,
    // pub(crate) animator: Animator,
    // pub(crate) frame_model: FrameModel,
    pub(crate) is_waiting_for_frame: bool,
    // pub(crate) security: crate::security::MicCameraStatus,
    pub(crate) batteries: Vec<BatteryDevice>,
    pub(crate) batteries_pristine: bool,
    // pub(crate) timer: Countdown,
    pub pill_container: Pill,
    pub(crate) pills_are_animating: bool

}

impl HeimdallrLayer {
    pub fn new (
        registry_state: RegistryState,
        output_state: OutputState,
        shm: Shm,
        config: Config
    ) -> Self {

        HeimdallrLayer {
            registry_state,
            output_state,
            shm,
            pool: None,
            layer: None,
            width: 1,
            height: 1,
            first_configure: true,
            // input_region: Some(empty_region),
            icons: HashMap::new(),
            ratatoskr_connected: false,
            // battery_integrated: None,
            needs_redraw: true,
            last_redraw: Instant::now(),
            redraw_interval: [Duration::from_millis(500), Duration::from_millis(60_000)],
            buffers: [None, None],
            current_buffer_idx: 0,
            config,
            notifications: vec![],
            // notification_idx: 0,
            wob_expiration: None,
            wob_value: TweenState::new(0.0),
            // animator: Animator::new(),
            // frame_model: FrameModel::new(),
            is_waiting_for_frame: false,
            // security: MicCameraStatus { mic_active: vec!(), camera_active: vec!(), pristine: false },
            batteries: vec![],
            batteries_pristine: false,
            // timer: Countdown::new(),
            pill_container: Pill::new(),
            pills_are_animating: false
        }
    }

    pub fn update_security_data (&mut self, data: MicCameraStatus) {
        self.pill_container.update_data_security(&data);
        // self.security = data; // TODO: Deprecated? Remove it?
    }

    pub fn update_battery_data (&mut self, data: Option<crate::battery::BatteryStats>) {
        // self.battery_integrated = data;
        if self.pill_container.update_data_battery(data, self.config.show_watts) {
            self.pill_container.recalculate_normal_target();
            self.request_redraw("pill_container animation");
        }
    }

    pub fn update_devices_data (&mut self, data: Vec<BatteryDevice>) {
        self.batteries = data;
        self.batteries_pristine = true;
        let _ = self.pill_container.update_data_devices(self.batteries.clone());
    }

    pub fn set_countdown (&mut self, input: &str) -> Result<u64, &'static str> {
        self.pill_container.set_countdown(input)
    }

    pub fn check_redraw_timeout(&mut self) {

        // if self.pill_container.is_countdown_active() && self.last_redraw.elapsed() > Duration::from_secs(1) {
        if self.pill_container.update_data_countdown() {
            self.request_redraw("timer tick");
        // } else if self.last_redraw.elapsed() > self.redraw_interval[1] {
        } else if self.pill_container.update_data_clock() {
            self.request_redraw("time");
        }
    }
    
    pub fn request_redraw(&mut self, _reason: &str) {
        self.needs_redraw = true;
        dbg_println!("{}", format!("Redraw requested by {}", _reason).yellow());
    }

    pub fn maybe_redraw(&mut self, qh: &QueueHandle<Self>) {

        // Update/prune notification history, then tick pill notification state
        self.update_notification_list(None);
        let (notif_changed, notif_needs_recalc) = self.pill_container.tick_notifications();
        if notif_changed {
            self.request_redraw("notification tick");
        }
        if notif_needs_recalc {
            self.pill_container.recalculate_normal_target();
        }

        if self.pill_container.needs_redraw || self.pill_container.needs_recalc {
            self.needs_redraw = true;
            eprintln!("=============== DIRTY PILL ============");
        } else {
            // eprintln!("=============== CLEAN PILL ============");
        }

        // Check if wob-like must be closed
        if let Some(exp) = self.wob_expiration {
            if Instant::now() > exp {
                self.wob_value.set_target(0.0);
                self.wob_expiration = None;
            }
        }

        let animating = self.wob_value.step() || self.pills_are_animating;
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

                // self.update_timer_icon();

                let cr = Context::new(&surface).unwrap();

                self.draw_myframe(cr.clone());

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

    fn draw_test_pill (&mut self, cr: &Context) {
        self.pills_are_animating = false;

        if self.pill_container.needs_recalc {
            dbg_println!("===============   RECALC   ============");
            self.pill_container.recalculate_normal_target();
        }


        // UPDATE ANIMATIONS
        if self.pill_container.step_animation() {
            self.pills_are_animating = true;
            self.request_redraw("pill_container animation");
        } else {
            // eprintln!("Pill container is NOT animating");
        }

        // OLD?
        

        let r = 8.0;
        let pill_bg_color: (f64, f64, f64, f64) = self.pill_container.get_bg_color();
        let mut pill_border_color: Option<(f64, f64, f64, f64)> = match self.config.frame_color {
            FrameColor::Rgba(r, g, b, a) => Some((r, g, b, a)),
            FrameColor::WorstResource => self
                .icons
                .values()
                .max_by(|a, b| a.warn.partial_cmp(&b.warn).unwrap_or(std::cmp::Ordering::Equal))
                .map(|icon| icon.color),
            FrameColor::None /* | FrameColor::Random */ => None
        };

        let (rect_width, rect_height) = self.pill_container.get_current_rect();
        let (rect_width_end, _rect_height_end) = self.pill_container.get_desired_rect();
        let rect_left = (self.width as f64 - rect_width) / 2.0;
        let rect_left_end = (self.width as f64 - rect_width_end) / 2.0;
        let rect_top = 2.0;

        let mut pill_bg_steps = vec![(0.0, pill_bg_color)];

        // wob-like
        let wob_ratio = self.wob_value.value();
        if wob_ratio > 0.0 {
            let wob_color_base = (0.6, 0.6, 0.7, pill_bg_color.3);
            let wob_color = mix_color(pill_bg_color, wob_color_base, wob_ratio.max(0.5));
            pill_border_color = Some(mix_color(pill_border_color.unwrap_or((0.0, 0.0, 0.0, 0.0)), wob_color_base, wob_ratio));
            let mut steps = vec![(0.0, wob_color)];
            steps.push((wob_ratio, pill_bg_color));
            pill_bg_steps = steps;
        }

        rounded_rect_gradient(&cr, rect_left, rect_top, rect_width, rect_height, r, pill_bg_steps, crate::utils::GradientDirection::Horizontal, false, pill_border_color);

        self.pill_container.draw(&cr, rect_width_end, rect_height, rect_left_end, rect_top);
    }

    fn draw_myframe(&mut self, cr: Context) {
        // cr.set_operator(cairo::Operator::Source);

        // Clear with full transparency
        // cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        cr.set_operator(cairo::Operator::Clear);
        cr.paint().unwrap();
        cr.set_operator(cairo::Operator::Over);

        // Draw rounded rectangle frame
        let thickness = 1.0;
        let radius = 25.0;
        let radius2 = 4.0;

        let w = self.width as f64;
        let h = self.height as f64;
        let w_hole = w - thickness - 2.0;

        let top = thickness / 2.0;

        // TODO: In the pill-ui, the hole will be always a rectangle, so we can use a simplified version of rounded_big_hole and remove the ReservedSpace stuff, don't we?
        // TODO: In the pill-ui we can also have rounded corners as separated surfaces? We lose the ability to have a border but we "lose" a lot of memory footprint too! But what if, in the future, pill will be able to expand vertically and host big component? We'll need potentially the entire screen in the buffer, just like now.

        // Outer black border + fill
        // rounded_big_hole(&cr, thickness / 2.0, top, w_hole, h - thickness - top, radius, radius2, res_w, res_h, wob_h);

        let spaces = vec![
            // ReservedSpace { anchor: Anchor::BottomRight, width: 100.0, height: 40.0 }
            // ReservedSpace { anchor: Anchor::BottomLeft, width: res_w, height: res_h }
            // ReservedSpace { anchor: Anchor::TopRight, width: 90.0, height: 20.0 }
        ];

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

    }

    pub fn update_notification_list (&mut self, new_notif_opt: Option<Notification>) -> bool {
        let mut changed: bool = false;
        if let Some(new_notif) = new_notif_opt {
            // Preserve replace/unmount handling in history
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

            // keep a history (most recent first)
            self.notifications.insert(0, new_notif.clone());

            // forward to pill which handles display rules/timing
            let (visible_changed, needs_recalc) = self.pill_container.push_notification(new_notif);
            changed = visible_changed || needs_recalc;
        }

        // prune expired history entries
        let a = self.notifications.len();
        self.notifications.retain(|n| n.expired_at.is_none() || (n.expired_at.unwrap() > Instant::now()));
        let b = self.notifications.len();
        changed = changed || (a != b);

        if changed {
            self.request_redraw("notifications updated");
        }

        changed

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
            if self.pill_container.update_data_warnings(&self.icons) {
                // self.pill_container.recalculate_normal_target();
                // self.request_redraw("pill_container animation");
            }
            IconChange::Changed
        } else {
            if self.pill_container.update_data_warnings(&self.icons) {
                // self.pill_container.recalculate_normal_target();
                // self.request_redraw("pill_container animation");
            }
            IconChange::Added
        }
    }

    pub fn remove_icon(&mut self, id: &str) -> bool {
        let removed = self.icons.remove(id).is_some();
        if removed {
            if self.pill_container.update_data_warnings(&self.icons) {
                // self.pill_container.recalculate_normal_target();
                // self.request_redraw("pill_container animation");
            }
        }
        removed
    }

    pub fn remove_notification(&mut self) -> bool {
        if self.notifications.len() > 0 {
            self.notifications.remove(0);
            self.pill_container.update_data_notifications(&self.notifications);
            true
        } else {
            false
        }
    }
    
    /* #[deprecated]
    pub fn show_notification(&mut self, new_idx: i32) -> bool {
        eprintln!("{} ({new_idx})", "Removed functionality".yellow());
        false
    } */

    pub fn show_value(&mut self, value: f64, _kind: Option<&str>) -> bool {
        let target = value.clamp(0.0, 1.0);
        let changed = self.wob_expiration.is_none() || (self.wob_value.target() - target).abs() > f64::EPSILON;
        self.wob_expiration = Some(Instant::now() + Duration::from_millis(2000));
        self.wob_value.set_target(target);
        if changed {
            self.request_redraw("wob value changed");
        }
        changed
    }
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