use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{globals::registry_queue_init, protocol::{wl_compositor, wl_region}, Connection};

use std::{time::{Duration, Instant}};

use smithay_client_toolkit::shell::WaylandSurface;

use std::collections::HashMap;

use crate::{data::{PartialMsg, HeimdallrSocket}, heimdallr_layer::screen_height};

mod data;
mod config;
mod heimdallr_layer;

use config::Config;

use crate::heimdallr_layer::HeimdallrLayer;


fn main() {
    env_logger::init();

    let config = Config::load_from_file("~/.config/heimdallr/config.json");
    println!("Configurazione caricata: {:?}", config);

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

    let pool = SlotPool::new(1920 * (screen_height as usize) * 4, &shm).expect("pool creation failed");

    //let resources = Arc::new(Mutex::new(ResourceData::default()));
    //let receiver = start_resource_watcher("/tmp/ratatoskr.json", resources.clone());

    // let rx = data::start_socket_watcher("/tmp/ratatoskr.sock");

    let mut app = HeimdallrLayer {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool,
        layer,
        width: 1920,
        height: screen_height,
        first_configure: true,
        input_region: Some(empty_region),
        icons: HashMap::new(),
        battery_eta: None,
        battery_recharging: None,
        needs_redraw: true,
        last_redraw: Instant::now(),
        redraw_interval: Duration::from_millis(1000),
        buffers: HashMap::new(),
        background_surface: None
    };
    
    // app.add_icon("avg", "󰬢", (1.0, 0.2, 0.2, 1.0)); // example
    // app.add_icon("ram", "󰘚", (1.0, 1.0, 0.2, 1.0)); // example

    // socket creation
    //let _ = std::fs::remove_file("/tmp/ratatoskr.sock");
    //let sock = UnixDatagram::bind("/tmp/ratatoskr.sock").expect("Impossible to create the socket in /tmp/ratatoskr.sock");
    //let mut buf = [0u8; 2048];
    //let state = Arc::new(Mutex::new(PartialMsg::default()));

    // let (tx, rx) = mpsc::channel();
    // start_socket_listener(Arc::clone(&state), tx);

    let mut sock = HeimdallrSocket::new("/tmp/ratatoskr.sock");

    loop {
        let _ = event_queue.dispatch_pending(&mut app);
        sock.poll_messages();

        // Prova a leggere nuovi eventi — non blocca
        match event_queue.prepare_read() {
            Some(mut guard) => {
                if let Err(e) = guard.read() {
                    // Silenzia WouldBlock (nessun evento da leggere)
                    /* if let Some(raw_err) = e.raw_os_error() {
                        if raw_err != 11 {
                            eprintln!("Wayland read() error: {:?}", e);
                        }
                    } */
                }
            }
            _ => {
                // Se non pronto a leggere, prova solo a flushare
                let _ = conn.flush();
            }
        }

        // Processa eventuali eventi appena arrivati
        let _ = event_queue.dispatch_pending(&mut app);

        // event_queue.blocking_dispatch(&mut app).unwrap();
        // while event_queue.dispatch_pending(&mut app).unwrap() > 0 {}


        // Locking read from socket
        //let len = sock.recv(&mut buf).unwrap();
        //let msg = String::from_utf8_lossy(&buf[..len]);
        //println!("Ricevuto: {}", msg);
        if let Ok(data) = sock.rx.try_recv() {
            //println!("Ricevuto: {:?}", data);
            if data.resource == "battery" {
                if let Some(bat) = &data.data {
                    // {"capacity": Number(177228.0), "color": String("#55FF00"), "eta": Number(380.0978088378906), "icon": String("\u{f0079}"), "percentage": Number(100), "state": String("Discharging"), "warn": Number(0.0), "watt": Number(7.76800012588501)}
                    // let old_eta = app.battery_eta;
                    // let old_state = app.battery_recharging;
                    app.battery_eta = bat["eta"].as_f64();
                    app.battery_recharging = match bat["state"].as_str().unwrap() {
                        "Discharging" => Some(false),
                        "Charging" => Some(true),
                        _ => None
                    };
                    app.request_redraw();
                    // eprintln!("{:?}", bat);
                    // eprintln!("battery {:?} {:?}", app.battery_recharging, app.battery_eta);
                }
            }

            if data.warning < 0.3 {
                if app.remove_icon(&data.resource) {
                    app.request_redraw();
                }
            }
            else {
                let mut icon = "";
                if data.resource == "loadavg" { icon = "󰬢"; }
                else if data.resource == "memory" { icon = "󰘚"; }
                else if data.resource == "temperature" { icon = &data.icon; }
                else if data.resource == "network" { icon = &data.icon; }

                if icon != "" {
                    app.remove_icon(&data.resource);
                    app.add_icon(&data.resource, icon, get_color_gradient(data.warning));
                    app.request_redraw();
                }
            }
        }

        app.maybe_redraw(&qh);
        conn.flush().unwrap();
        std::thread::sleep(Duration::from_millis(10));
    }

/*
use std::os::unix::net::UnixDatagram;

fn main() {
    let sock = UnixDatagram::unbound().unwrap();
    sock.connect("/tmp/ratatoskr.sock").unwrap();

    let mut buf = [0u8; 2048];
    loop {
        let len = sock.recv(&mut buf).unwrap();
        let msg = String::from_utf8_lossy(&buf[..len]);
        println!("Ricevuto: {}", msg);
    }
} */
}

/* fn on_icons_changed(&mut self) {
    self.redraw();
} */

const DEFAULT_WHITE: bool = false;
pub fn get_color_gradient(value: f64) -> (f64, f64, f64, f64) {
    get_color_gradient_full(0.0, 1.0, value, false)
}
pub fn get_color_gradient_full(min: f64, max: f64, value: f64, reversed: bool) -> (f64, f64, f64, f64) {
    let clamped = value.clamp(min, max);
    let mut ratio = if (max - min).abs() < f64::EPSILON {
        0.5
    } else {
        (clamped - min) / (max - min)
    };

    if !reversed { ratio = 1.0 - ratio; }
    let sat;
    let hue;
    if DEFAULT_WHITE {
        sat = f64::max(1.0 - (ratio * ratio * ratio), 0.0);
        hue = 60.0 * ratio; // 60 -> 0
    } else {
        sat = 1.0;
        hue = 100.0 * ratio; // 100 -> 0
    }
    let (r, g, b) = hsv_to_rgb(hue, sat, 1.0);

    // format!("#{:02X}{:02X}{:02X}", r, g, b)
    ((r as f64) / 255.0, (g as f64) / 255.0, (b as f64) / 255.0, 1.0)
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    let r = ((r1 + m) * 255.0).round() as u8;
    let g = ((g1 + m) * 255.0).round() as u8;
    let b = ((b1 + m) * 255.0).round() as u8;

    (r, g, b)
}