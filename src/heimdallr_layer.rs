use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState}, delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm, output::{OutputHandler, OutputState}, registry::{ProvidesRegistryState, RegistryState}, registry_handlers, shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure}, shm::{Shm, ShmHandler, slot::{Buffer, SlotPool}}
};
use wayland_client::{Connection, QueueHandle, protocol::{wl_compositor, wl_output::WlOutput, wl_region, wl_shm}};
use cairo::{Context, Format, ImageSurface, FontSlant};

use std::time::{Duration, Instant};

use smithay_client_toolkit::shell::WaylandSurface;

use std::collections::HashMap;

use wayland_client::Dispatch;
use colored::Colorize;

use crate::{config::{Config, FrameColor}, data::{AlarmIcon, BatteryDevice, IconChange}, dbg_println, niri::{WindowInfo, WorkspaceInfo}, notifications::Notification, pills::{Pill, PillModuleTrait}, security::MicCameraStatus, utils::{TweenState, log_to_file, mix_color, rounded_rect_gradient}};

const CORNER_SIZE: u32 = 32;
const PILL_SURFACE_WIDTH: u32 = 1200;
const PILL_SURFACE_HEIGHT: u32 = 128;
const WORKSPACE_SURFACE_WIDTH: u32 = 60;
const WORKSPACE_SURFACE_HEIGHT: u32 = 200;

#[derive(Clone, Copy)]
enum Corner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

struct RenderSurface {
    layer: LayerSurface,
    pool: Option<SlotPool>,
    width: u32,
    height: u32,
    buffers: [Option<Buffer>; 2],
    current_buffer_idx: usize,
    configured: bool,
    waiting_for_frame: bool,
    corner: Option<Corner>,
}

impl RenderSurface {
    fn new(layer: LayerSurface, width: u32, height: u32, corner: Option<Corner>) -> Self {
        Self {
            layer,
            pool: None,
            width,
            height,
            buffers: [None, None],
            current_buffer_idx: 0,
            configured: false,
            waiting_for_frame: false,
            corner,
        }
    }

    fn matches(&self, layer: &LayerSurface) -> bool {
        self.layer.wl_surface() == layer.wl_surface()
    }

    fn matches_surface(&self, surface: &wayland_client::protocol::wl_surface::WlSurface) -> bool {
        self.layer.wl_surface() == surface
    }

    fn configure(&mut self, width: u32, height: u32, shm: &Shm) {
        self.width = if width == 0 { self.width } else { width };
        self.height = if height == 0 { self.height } else { height };
        self.pool = Some(SlotPool::new((self.width * self.height * 4) as usize, shm).expect("pool creation failed"));
        self.buffers = [None, None];
        self.current_buffer_idx = 0;
        self.configured = true;
    }
}

pub struct HeimdallrLayer {
    pub(crate) registry_state: RegistryState,
    pub(crate) output_state: OutputState,
    pub(crate) shm: Shm,
    pill_surface: Option<RenderSurface>,
    workspace_surface: Option<RenderSurface>,
    corner_surfaces: Vec<RenderSurface>,
    // pub(crate) input_region: Option<wl_region::WlRegion>,
    pub(crate) icons: HashMap<String, AlarmIcon>,
    // pub(crate) battery_integrated: Option<crate::battery::BatteryStats>,
    pub(crate) needs_redraw: bool,
    pub(crate) last_redraw: Instant,
    pub(crate) redraw_interval: [Duration; 2],
    pub(crate) config: crate::config::Config,
    pub(crate) notifications: Vec<crate::notifications::Notification>,
    pub(crate) wob_value: TweenState,
    pub(crate) wob_expiration: Option<Instant>,
    pub(crate) ratatoskr_connected: bool,
    // pub(crate) animator: Animator,
    // pub(crate) frame_model: FrameModel,
    // pub(crate) security: crate::security::MicCameraStatus,
    pub(crate) batteries: Vec<BatteryDevice>,
    pub(crate) batteries_pristine: bool,
    // pub(crate) timer: Countdown,
    pub pill_container: Pill,
    pub(crate) pills_are_animating: bool,

    pub(crate) workspaces: Vec<WorkspaceInfo>,
    workspaces_opacity: TweenState,
    workspace_expiration: Option<Instant>
}

impl HeimdallrLayer {
    pub fn new (
        registry_state: RegistryState,
        output_state: OutputState,
        shm: Shm,
        config: Config
    ) -> Self {
        let pill = Pill::new(&config);

        HeimdallrLayer {
            registry_state,
            output_state,
            shm,
            pill_surface: None,
            corner_surfaces: Vec::new(),
            // input_region: Some(empty_region),
            icons: HashMap::new(),
            ratatoskr_connected: false,
            // battery_integrated: None,
            needs_redraw: true,
            last_redraw: Instant::now(),
            redraw_interval: [Duration::from_millis(500), Duration::from_millis(60_000)],
            config,
            notifications: vec![],
            // notification_idx: 0,
            wob_expiration: None,
            wob_value: TweenState::new(0.0),
            // animator: Animator::new(),
            // frame_model: FrameModel::new(),
            // security: MicCameraStatus { mic_active: vec!(), camera_active: vec!(), pristine: false },
            batteries: vec![],
            batteries_pristine: false,
            // timer: Countdown::new(),
            pill_container: pill,
            pills_are_animating: false,
            workspaces: vec![],
            workspace_surface: None,
            workspaces_opacity: TweenState::new(0.0),
            workspace_expiration: None,
        }
    }

    pub fn update_security_data (&mut self, data: MicCameraStatus) {
        self.pill_container.update_data_security(&data);
        // self.security = data; // TODO: Deprecated? Remove it?
    }

    pub fn update_niri_data (&mut self, data: Vec<WindowInfo>) {
        if self.pill_container.update_data_niri(data) {
            // self.pill_container.recalculate_normal_target();
            // self.request_redraw("pill_container animation (niri)");
        }
    }

    pub fn update_workspaces(&mut self, data: Vec<WorkspaceInfo>) {
        self.workspaces = data;
        self.workspaces_opacity.set_target(1.0);
        self.workspace_expiration = Some(Instant::now() + Duration::from_millis(2000));
        self.request_redraw("workspaces updated");
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

        // Check if workspaces must be closed
        if let Some(exp) = self.workspace_expiration {
            if Instant::now() > exp {
                self.workspaces_opacity.set_target(0.0);
                self.workspace_expiration = None;
            }
        }

        let animating = self.wob_value.step() || self.workspaces_opacity.step() || self.pills_are_animating;
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
        self.draw_workspaces(qh);
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
        let Some(mut surface) = self.pill_surface.take() else { return; };
        if !surface.configured || surface.waiting_for_frame {
            self.pill_surface = Some(surface);
            return;
        }

        let Some(pool) = surface.pool.as_mut() else {
            self.pill_surface = Some(surface);
            return;
        };
        let buffer_idx = Self::acquire_buffer(&mut surface.buffers, surface.width, surface.height, surface.current_buffer_idx, pool);
        let Some(buffer_idx) = buffer_idx else {
            self.pill_surface = Some(surface);
            return;
        };
        let buffer = surface.buffers[buffer_idx].as_ref().unwrap();
        let canvas = buffer.canvas(pool).expect("canvas should be available immediately");
        let image = unsafe {
            ImageSurface::create_for_data_unsafe(canvas.as_mut_ptr(), Format::ARgb32, surface.width as i32, surface.height as i32, buffer.stride()).unwrap()
        };
        let cr = Context::new(&image).unwrap();
        cr.set_operator(cairo::Operator::Clear);
        cr.paint().unwrap();
        cr.set_operator(cairo::Operator::Over);
        self.draw_test_pill(&cr, surface.width);

        buffer.attach_to(surface.layer.wl_surface()).unwrap();
        surface.layer.wl_surface().damage_buffer(0, 0, surface.width as i32, surface.height as i32);
        surface.waiting_for_frame = true;
        surface.layer.wl_surface().frame(qh, surface.layer.wl_surface().clone());
        surface.layer.commit();
        drop(image);
        surface.current_buffer_idx = (buffer_idx + 1) % surface.buffers.len();
        self.last_redraw = Instant::now();
        self.pill_surface = Some(surface);
    }

    fn draw_workspaces(&mut self, qh: &QueueHandle<Self>) {

        let opacity = self.workspaces_opacity.value().clamp(0.0, 1.0);
        // if opacity <= 0.0 { return; } // ! To avoid or not to avoid? Ensure surface cleaning (line 366/367) before returning?

        // dbg_println!("\n==== DRAW WORKSPACES 1 ====\n");
        let Some(mut surface) = self.workspace_surface.take() else { return; };
        // dbg_println!("\n==== DRAW WORKSPACES 1.0 ==== {} {}\n", surface.configured, surface.waiting_for_frame);
        if !surface.configured || surface.waiting_for_frame {
            self.workspace_surface = Some(surface);
            return;
        }

        // dbg_println!("\n==== DRAW WORKSPACES 1.1 ====\n");

        let Some(pool) = surface.pool.as_mut() else {
            self.workspace_surface = Some(surface);
            return;
        };
        // dbg_println!("\n==== DRAW WORKSPACES 1.2 ====\n");
        let buffer_idx = Self::acquire_buffer(&mut surface.buffers, surface.width, surface.height, surface.current_buffer_idx, pool);
        let Some(buffer_idx) = buffer_idx else {
            self.workspace_surface = Some(surface);
            return;
        };
        // dbg_println!("\n==== DRAW WORKSPACES 1.3 ====\n");
        let buffer = surface.buffers[buffer_idx].as_ref().unwrap();
        let canvas = buffer.canvas(pool).expect("workspace canvas should be available");
        let image = unsafe {
            ImageSurface::create_for_data_unsafe(canvas.as_mut_ptr(), Format::ARgb32, surface.width as i32, surface.height as i32, buffer.stride()).unwrap()
        };
        let cr = Context::new(&image).unwrap();
        cr.set_operator(cairo::Operator::Clear);
        cr.paint().unwrap();
        cr.set_operator(cairo::Operator::Over);

        // cr.select_font_face("", FontSlant::Normal, cairo::FontWeight::Normal);
        // cr.set_font_size(14.0);

        let item_h: f64 = 14.0;
        let count = self.workspaces.len().max(1);
        let total_h = item_h * (count as f64);
        let mut y = ((surface.height as f64) - total_h) / 2.0 + item_h/2.0;

        // dbg_println!("\n==== DRAW WORKSPACES 2 ====\n");

        self.workspaces.sort_by_key(|ws| ws.output.clone());

        let mut last_output: Option<String> = None;
        for ws in &self.workspaces {
            if let Some(last) = &last_output {
                if last != &ws.output {
                    y += item_h/2.0;
                }
            }
            // let circle_x = 12.0;
            // let circle_y = y;
            let mut /*(r,g,b,a)*/ color = 
                if ws.is_urgent { (1.0, 0.2, 0.2, 0.7) } else 
                if ws.is_focused { (0.9, 0.4, 0.3, 0.7) } else 
                if ws.is_active { (0.8, 0.6, 0.4, 0.7) } else 
                { (0.6, 0.6, 0.6, 0.7) };

            color.3 *= opacity;
            /* cr.set_source_rgba(r,g,b,a);
            cr.arc(circle_x, circle_y, 6.0, 0.0, std::f64::consts::PI * 2.0);
            cr.fill().unwrap(); */

            /* let name = ws.name.clone().unwrap_or_else(|| format!("{}", ws.idx));
            cr.set_source_rgba(1.0,1.0,1.0,1.0);
            let tx = 30.0;
            // Align text vertically roughly centered
            cr.move_to(tx, y + 5.0);
            cr.show_text(&name).ok(); */

            let rect_top = y;
            let rect_width = 20.0 + 5.0 * (ws.window_count as f64).min(3.0);
            let rect_left = (WORKSPACE_SURFACE_WIDTH as f64) - 5.0 - rect_width;
            let rect_height = 4.0;
            let pill_bg_steps = vec![(0.0, color)];
            let pill_border_color = Some((0.0, 0.0, 0.0, 0.4 * opacity)); // Some((r,g,b,1.0)); // if ws.is_focused { Some((1.0, 0.4, 0.3, 1.0)) } else { None };
            let radius = 2.0;
            rounded_rect_gradient(&cr, rect_left, rect_top, rect_width, rect_height, radius, pill_bg_steps, crate::utils::GradientDirection::Horizontal, false, pill_border_color);

            /* for i in 0..ws.window_count.min(5) {
                let circle_x = rect_left - 5.0 - 5.0 * (i as f64);
                let circle_y = rect_top + rect_height / 2.0;
                // let (r,g,b,a) = if ws.is_urgent { (1.0, 0.2, 0.2, 0.7) } else { (0.6, 0.6, 0.6, 0.7) };
                cr.set_source_rgba(color.0,color.1,color.2,color.3);
                cr.arc(circle_x, circle_y, rect_height / 2.0, 0.0, std::f64::consts::PI * 2.0);
            }
            cr.fill().unwrap(); */

            y += item_h;
            last_output = Some(ws.output.clone());
            // dbg_println!("{}", format!("\n==== DRAW WORKSPACES 2.{} ====\n", ws.idx));
        }

        buffer.attach_to(surface.layer.wl_surface()).unwrap();
        surface.layer.wl_surface().damage_buffer(0, 0, surface.width as i32, surface.height as i32);
        surface.waiting_for_frame = true;
        surface.layer.wl_surface().frame(qh, surface.layer.wl_surface().clone());
        surface.layer.commit();
        drop(image);
        surface.current_buffer_idx = (buffer_idx + 1) % surface.buffers.len();
        self.last_redraw = Instant::now();
        self.workspace_surface = Some(surface);
    }

    fn draw_test_pill (&mut self, cr: &Context, surface_width: u32) {
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
        let mut pill_border_color: Option<(f64, f64, f64, f64)> = match self.config.pill_border_color {
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
        let rect_left = (surface_width as f64 - rect_width) / 2.0;
        let rect_left_end = (surface_width as f64 - rect_width_end) / 2.0;
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

    fn draw_corner(cr: &Context, corner: Corner) {
        let size = CORNER_SIZE as f64;
        cr.set_operator(cairo::Operator::Clear);
        cr.paint().unwrap();
        cr.set_operator(cairo::Operator::Over);
        cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
        cr.rectangle(0.0, 0.0, size, size);
        cr.fill().unwrap();

        let (x, y, start, end) = match corner {
            Corner::TopLeft => (size, size, std::f64::consts::PI, 1.5 * std::f64::consts::PI),
            Corner::TopRight => (0.0, size, 1.5 * std::f64::consts::PI, 2.0 * std::f64::consts::PI),
            Corner::BottomRight => (0.0, 0.0, 0.0, 0.5 * std::f64::consts::PI),
            Corner::BottomLeft => (size, 0.0, 0.5 * std::f64::consts::PI, std::f64::consts::PI),
        };
        cr.set_operator(cairo::Operator::Clear);
        cr.arc(x, y, size, start, end);
        cr.line_to(x, y);
        cr.close_path();
        cr.fill().unwrap();
        cr.set_operator(cairo::Operator::Over);
    }

    fn draw_static_surface(surface: &mut RenderSurface) {
        let Some(pool) = surface.pool.as_mut() else { return; };
        let buffer_idx = Self::acquire_buffer(&mut surface.buffers, surface.width, surface.height, surface.current_buffer_idx, pool);
        let Some(buffer_idx) = buffer_idx else { return; };
        let buffer = surface.buffers[buffer_idx].as_ref().unwrap();
        let canvas = buffer.canvas(pool).expect("corner canvas should be available");
        let image = unsafe {
            ImageSurface::create_for_data_unsafe(canvas.as_mut_ptr(), Format::ARgb32, surface.width as i32, surface.height as i32, buffer.stride()).unwrap()
        };
        let cr = Context::new(&image).unwrap();
        Self::draw_corner(&cr, surface.corner.unwrap());
        buffer.attach_to(surface.layer.wl_surface()).unwrap();
        surface.layer.wl_surface().damage_buffer(0, 0, surface.width as i32, surface.height as i32);
        surface.layer.commit();
        drop(image);
        surface.current_buffer_idx = (buffer_idx + 1) % surface.buffers.len();
    }

    pub fn install_surfaces(
        &mut self,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        qh: &QueueHandle<Self>,
        output: Option<&WlOutput>,
        raw_compositor: &wl_compositor::WlCompositor,
    ) {
        let empty_region = raw_compositor.create_region(qh, ());
        let corners = [
            (Corner::TopLeft, Anchor::TOP | Anchor::LEFT),
            (Corner::TopRight, Anchor::TOP | Anchor::RIGHT),
            (Corner::BottomRight, Anchor::BOTTOM | Anchor::RIGHT),
            (Corner::BottomLeft, Anchor::BOTTOM | Anchor::LEFT),
        ];
        self.corner_surfaces = corners.into_iter().map(|(corner, anchor)| {
            let surface = compositor.create_surface(qh);
            let layer = layer_shell.create_layer_surface(qh, surface, Layer::Overlay, Some("heimdallr-corner"), output);
            layer.set_anchor(anchor);
            layer.set_size(CORNER_SIZE, CORNER_SIZE);
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            layer.wl_surface().set_input_region(Some(&empty_region));
            layer.commit();
            RenderSurface::new(layer, CORNER_SIZE, CORNER_SIZE, Some(corner))
        }).collect();

        let surface = compositor.create_surface(qh);
        let layer = layer_shell.create_layer_surface(qh, surface, Layer::Overlay, Some("heimdallr-pill"), output);
        layer.set_anchor(Anchor::TOP);
        layer.set_size(PILL_SURFACE_WIDTH, PILL_SURFACE_HEIGHT);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.wl_surface().set_input_region(Some(&empty_region));
        layer.commit();
        self.pill_surface = Some(RenderSurface::new(layer, PILL_SURFACE_WIDTH, PILL_SURFACE_HEIGHT, None));

        // Workspace indicator surface (right margin, centered vertically)
        let surface = compositor.create_surface(qh);
        let layer = layer_shell.create_layer_surface(qh, surface, Layer::Overlay, Some("heimdallr-workspaces"), output);
        layer.set_anchor(Anchor::RIGHT);
        layer.set_size(WORKSPACE_SURFACE_WIDTH, WORKSPACE_SURFACE_HEIGHT);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.wl_surface().set_input_region(Some(&empty_region));
        layer.commit();
        self.workspace_surface = Some(RenderSurface::new(layer, WORKSPACE_SURFACE_WIDTH, WORKSPACE_SURFACE_HEIGHT, None));
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
        // First attempt to dismiss the currently displayed notification in the pill
        if self.pill_container.dismiss_current_notification() {
            self.request_redraw("notification dismissed");
            return true;
        }

        // Fallback: remove from history if any
        // TODO: To be removed?
        if self.notifications.len() > 0 {
            self.notifications.remove(0);
            let _ = self.pill_container.update_data_notifications(&self.notifications);
            self.request_redraw("notification removed from history");
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
    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, surface: &wayland_client::protocol::wl_surface::WlSurface, _: u32) {
        dbg_println!("SCTK Frame callback received");
        if let Some(pill) = self.pill_surface.as_mut() {
            if pill.matches_surface(surface) {
                pill.waiting_for_frame = false;
                self.maybe_redraw(qh);
                return;
            }
        }

        if let Some(ws) = self.workspace_surface.as_mut() {
            if ws.matches_surface(surface) {
                ws.waiting_for_frame = false;
                self.maybe_redraw(qh);
                return;
            }
        }
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

    fn configure(&mut self, _: &Connection, qh: &QueueHandle<Self>, layer: &LayerSurface, configure: LayerSurfaceConfigure, _: u32) {
        eprintln!("LayerShell surface configured by compositor {:?}", configure.new_size);

        if let Some(pill) = self.pill_surface.as_mut() {
            if pill.matches(layer) {
                pill.configure(configure.new_size.0, configure.new_size.1, &self.shm);
                self.draw(qh);
                self.draw_workspaces(qh);
                return;
            }
        }

        if let Some(ws) = self.workspace_surface.as_mut() {
            if ws.matches(layer) {
                ws.configure(configure.new_size.0, configure.new_size.1, &self.shm);
                self.draw_workspaces(qh);
                return;
            }
        }

        for corner in &mut self.corner_surfaces {
            if corner.matches(layer) {
                corner.configure(configure.new_size.0, configure.new_size.1, &self.shm);
                Self::draw_static_surface(corner);
                return;
            }
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