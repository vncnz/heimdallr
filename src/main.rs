use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{globals::registry_queue_init, protocol::{wl_shm, wl_compositor, wl_region}, Connection, QueueHandle};
use cairo::{Context, Format, ImageSurface};

use std::{num::NonZeroU32};

use smithay_client_toolkit::shell::WaylandSurface;

use std::collections::HashMap;
use cairo::FontSlant;

struct AlarmIcon {
    symbol: String,
    color: (f64, f64, f64, f64), // RGBA
}



fn main() {
    env_logger::init();

    let conn = Connection::connect_to_env().unwrap();
    let (globals, mut event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).unwrap();
    let layer_shell = LayerShell::bind(&globals, &qh).unwrap();
    let shm = Shm::bind(&globals, &qh).unwrap();

    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some("heimdallr"), None);
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);

    let raw_compositor: wl_compositor::WlCompositor =
    globals.bind::<wl_compositor::WlCompositor, _, _>(&qh, 1..=4, ())
    .expect("failed to bind wl_compositor for region creation");

    let empty_region: wl_region::WlRegion = raw_compositor.create_region(&qh, ());
    layer.wl_surface().set_input_region(Some(&empty_region));
    
    layer.set_size(0, 0); // full screen
    layer.commit();

    let pool = SlotPool::new(1920 * 1080 * 4, &shm).expect("pool creation failed");

    let mut app = HeimdallrLayer {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool,
        layer,
        width: 1920,
        height: 1080,
        first_configure: true,
        input_region: Some(empty_region),
        icons: HashMap::new(),
    };
    app.add_icon("avg", "󰬢", (1.0, 0.2, 0.2, 1.0)); // example
    app.add_icon("ram", "󰘚", (1.0, 1.0, 0.2, 1.0)); // example

    loop {
        event_queue.blocking_dispatch(&mut app).unwrap();
    }
}

/* fn on_icons_changed(&mut self) {
    self.redraw();
} */

struct HeimdallrLayer {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    layer: LayerSurface,
    width: u32,
    height: u32,
    first_configure: bool,
    input_region: Option<wl_region::WlRegion>,
    icons: HashMap<String, AlarmIcon>,
}

impl HeimdallrLayer {
    fn draw(&mut self, qh: &QueueHandle<Self>) {
        let stride = self.width as i32 * 4;
        let (buffer, mut canvas) = self
            .pool
            .create_buffer(self.width as i32, self.height as i32, stride, wl_shm::Format::Argb8888)
            .expect("buffer creation failed");

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

        // Clear with full transparency
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        cr.paint().unwrap();

        // icons space reserved
        let mut y_offset = self.height as f64 - 12.0; // parte dal basso
        let res_w = 30.0;
        let res_h = (self.icons.len() as f64) * 30.0 + 6.0;

        // Draw rounded rectangle frame
        let thickness = 1.0;
        let radius = 25.0;
        let radius2 = 10.0;

        let w = self.width as f64;
        let h = self.height as f64;

        // Outer black border (semi-transparent)
        cr.rectangle(0.0, 0.0, w, h);
        cr.set_fill_rule(cairo::FillRule::EvenOdd);
        rounded_rect(&cr, thickness / 2.0, thickness / 2.0, w - thickness, h - thickness, radius, radius2, res_w, res_h);
        cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
        cr.fill().unwrap();

        //cr.set_line_width(thickness);
        //cr.set_source_rgba(0.0, 0.0, 0.0, 0.8);
        //rounded_rect(&cr, thickness / 2.0, thickness / 2.0, w - thickness, h - thickness, radius);
        //cr.stroke().unwrap();

        // Inner colored frame (for future dynamic border)
        cr.set_line_width(1.0);
        cr.set_source_rgba(0.2, 0.6, 1.0, 0.9);
        rounded_rect(&cr, thickness / 2.0, thickness / 2.0, w - thickness, h - thickness, radius, radius2, res_w, res_h);
        cr.stroke().unwrap();

        // Damage + commit
        self.layer.wl_surface().damage_buffer(0, 0, self.width as i32, self.height as i32);
        self.layer.wl_surface().frame(qh, self.layer.wl_surface().clone());
        buffer.attach_to(self.layer.wl_surface()).unwrap();

        // === Draw alarm icons ===
        
        for icon in self.icons.values() {
            cr.select_font_face("Symbols Nerd Font Mono", FontSlant::Normal, cairo::FontWeight::Normal);
            cr.set_font_size(20.0);

            cr.set_source_rgba(icon.color.0, icon.color.1, icon.color.2, icon.color.3);
            cr.move_to(4.0, y_offset);
            cr.show_text(&icon.symbol).unwrap();
            y_offset -= 30.0;
        }

        self.layer.commit();
    }
}

impl HeimdallrLayer { // This is for icon management, I like to keep it separated, for now
    fn add_icon(&mut self, id: &str, symbol: &str, color: (f64, f64, f64, f64)) {
        self.icons.insert(
            id.to_string(),
            AlarmIcon {
                symbol: symbol.to_string(),
                color,
            },
        );
    }

    fn remove_icon(&mut self, id: &str) {
        self.icons.remove(id);
    }
}

fn rounded_rect(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64, r2: f64, reserved_w: f64, reserved_h: f64) {
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -90f64.to_radians(), 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, 90f64.to_radians());
    
    if reserved_h > 0.0 {
        cr.arc(x + r + reserved_w, y + h - r, r, 90f64.to_radians(), 180f64.to_radians());
        cr.arc_negative(x - r2 + reserved_w, y + h + r2 - reserved_h, r2, 0f64.to_radians(), 270f64.to_radians());
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
        self.draw(qh);
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

use wayland_client::{Dispatch};

impl Dispatch<wl_compositor::WlCompositor, ()> for HeimdallrLayer {
    fn event(
        _state: &mut Self,
        _proxy: &wl_compositor::WlCompositor,
        _event: wl_compositor::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
        // wl_compositor non genera eventi interessanti per noi → no-op
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
        // wl_region non genera eventi interessanti → no-op
    }
}