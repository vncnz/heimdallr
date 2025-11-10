
use smithay_client_toolkit::{
    compositor::{CompositorState},
    output::{OutputState},
    registry::{RegistryState},
    shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
    shm::{slot::SlotPool, Shm},
};
use wayland_client::{globals::registry_queue_init, protocol::{wl_compositor, wl_region}, Connection};

use std::{sync::mpsc::{self, Receiver, Sender}, time::{Duration, Instant}};

use smithay_client_toolkit::shell::WaylandSurface;

use std::collections::HashMap;

use crate::{commands::start_command_listener, data::HeimdallrSocket, heimdallr_layer::screen_height, notifications::Notification, utils::get_color_gradient};

mod data;
mod config;
mod heimdallr_layer;
mod notifications;
mod commands;
mod utils;

use config::Config;

use crate::heimdallr_layer::HeimdallrLayer;
use crate::notifications::start_notification_listener;

// Tip: find src | entr -r cargo run for a sorta hotreloading (entr is an external cmd to be installed using pacman)

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
        ratatoskr_connected: false,
        battery_eta: None,
        battery_recharging: None,
        needs_redraw: true,
        last_redraw: Instant::now(),
        redraw_interval: Duration::from_millis(1000),
        buffers: HashMap::new(),
        background_surface: None,
        config,
        notifications: vec![],
        notification_idx: 0
    };
    
    // app.add_icon("avg", "󰬢", (1.0, 0.2, 0.2, 1.0)); // example
    let (tx, rx_cmds): (Sender<String>, Receiver<String>) = mpsc::channel();
    let _ = start_command_listener(tx, "/tmp/heimdallr_cmds");

    let mut sock = HeimdallrSocket::new("/tmp/ratatoskr.sock");

    let (tx, rx_notif): (Sender<Vec<Notification>>, Receiver<Vec<Notification>>) = mpsc::channel();
    // let rx_notif: Option<Receiver<Notification>> = None;
    use std::thread;
    thread::spawn(|| {
        futures::executor::block_on(async {
            if let Err(e) = start_notification_listener(tx).await {
                eprintln!("Notification listener error: {:?}", e);
            }
        });
    });
    // let rx_notif = start_notification_listener().await;

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

        if let Ok(cmd) = rx_cmds.try_recv() {
            match &*cmd {
                "hide_notification" => {
                    println!("hide!");
                    if app.remove_notification() {
                        app.request_redraw();
                    }
                },
                "prev_notification" => {
                    println!("prev!");
                    if app.show_notification(app.notification_idx as i32 - 1) {
                        app.request_redraw();
                    }
                },
                "next_notification" => {
                    println!("next!");
                    if app.show_notification(app.notification_idx as i32 + 1) {
                        app.request_redraw();
                    }
                },
                _ => println!("{}", cmd)
            };
            // app.request_redraw();
        }
        
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

            let new_ratatoskr_status = data.warning < 0.5;
            if app.ratatoskr_connected != new_ratatoskr_status {
                app.ratatoskr_connected = new_ratatoskr_status;
                app.request_redraw();
            }
            if data.warning < 0.3 {
                if app.remove_icon(&data.resource) {
                    app.request_redraw();
                }
            }
            else {
                let mut icon = "";
                if data.resource == "loadavg" { icon = "󰬢"; }
                else if data.resource == "ram" { icon = "󰘚"; }
                else if data.resource == "temperature" { icon = &data.icon; }
                else if data.resource == "network" { icon = &data.icon; }
                else if data.resource == "disk" { icon = "󰋊"; }
                // weather
                // volume
                // disk
                // display

                if icon != "" {
                    app.remove_icon(&data.resource);
                    app.add_icon(&data.resource, icon, get_color_gradient(data.warning));
                    app.request_redraw();
                }
            }
        }

        if let Ok(list) = rx_notif.try_recv() {
            // draw_notifications(&list);
            // manage redraw
            println!("{:?}", list);
            app.notifications = list;
            app.request_redraw();
        }

        app.maybe_redraw(&qh);
        conn.flush().unwrap();
        std::thread::sleep(Duration::from_millis(10));
    }
}
