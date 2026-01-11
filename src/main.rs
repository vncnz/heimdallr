use signal_hook::{consts::{SIGHUP, SIGINT, SIGPIPE, SIGTERM}, iterator::Signals, low_level::signal_name};

use serde::Deserialize;
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

use std::panic;

use crate::{commands::start_command_listener, data::{BluetoothStats, DeviceKind, RatatoskrSocket}, notifications::Notification, utils::{AnimationKey, Animator, FrameModel, get_color_gradient, log_to_file, select_icon}};

mod data;
mod config;
mod heimdallr_layer;
mod notifications;
mod commands;
mod utils;
use config::Config;
// use chrono;

use crate::heimdallr_layer::HeimdallrLayer;
use crate::notifications::start_notification_listener;

// Tip: find src | entr -r cargo run for a sorta hotreloading (entr is an external cmd to be installed using pacman)

fn main() {
    panic::set_hook(Box::new(|info| {
        eprintln!("PANIC");
        eprintln!("{info}");
        let bt = std::backtrace::Backtrace::capture();
        eprintln!("{bt}");

        log_to_file("PANIC".to_string());
        log_to_file(format!("{info}"));
        log_to_file(format!("{bt}"));
    }));

    let mut signals = Signals::new([SIGTERM, SIGINT, SIGHUP, SIGPIPE]).unwrap();
    std::thread::spawn(move || {
        for sig in signals.forever() {
            let name = signal_name(sig).unwrap_or("UNKNOWN");
            eprintln!("Received signal {sig} ({name}) from the system");
            log_to_file("Received signal {sig} ({name}) from the system".to_string());

            match sig {
                SIGPIPE | SIGHUP => {
                    // log only
                }

                SIGINT | SIGTERM => {
                    eprintln!("Graceful shutdown requested");
                    std::process::exit(0);
                }

                /* SIGABRT | SIGSEGV => {
                    eprintln!("Fatal signal {}, aborting", sig);
                    std::process::abort(); // preserve core dump
                } */

                _ => {} // Dummy case
            }
        }
    });

    env_logger::init();

    let config = Config::load_from_file("~/.config/heimdallr/config.json");
    log_to_file(format!("Configurazione caricata: {:?}", config));

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
        height: 1080,
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
        config: config.clone(),
        notifications: vec![],
        notification_idx: 0,
        animator: Animator::new(),
        frame_model: FrameModel::new()
    };
    
    // app.add_icon("avg", "󰬢", (1.0, 0.2, 0.2, 1.0)); // example
    let (tx, rx_cmds): (Sender<String>, Receiver<String>) = mpsc::channel();
    let _ = start_command_listener(tx, "/tmp/heimdallr_cmds");

    let mut sock = RatatoskrSocket::new("/tmp/ratatoskr.sock");

    let (tx, rx_notif): (Sender<Vec<Notification>>, Receiver<Vec<Notification>>) = mpsc::channel();
    // let rx_notif: Option<Receiver<Notification>> = None;
    use std::thread;
    thread::spawn(|| {
        futures::executor::block_on(async {
            if let Err(e) = start_notification_listener(tx).await {
                log_to_file(format!("Notification listener error: {:?}", e));
            }
        });
    });
    // let rx_notif = start_notification_listener().await;

    loop {
        let _ = event_queue.dispatch_pending(&mut app);
        sock.poll_messages();

        // Prova a leggere nuovi eventi — non blocca
        match event_queue.prepare_read() {
            Some(guard) => {
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

        // Dispatch wayland events
        let _ = event_queue.dispatch_pending(&mut app);

        if let Ok(cmd) = rx_cmds.try_recv() {
            match &*cmd {
                "hide_notification" => {
                    println!("hide!");
                    if app.remove_notification() {
                        app.request_redraw("hide_notification");
                    }
                },
                "prev_notification" => {
                    println!("prev!");
                    if app.show_notification(app.notification_idx as i32 - 1) {
                        app.request_redraw("prev_notification");
                    }
                },
                "next_notification" => {
                    println!("next!");
                    if app.show_notification(app.notification_idx as i32 + 1) {
                        app.request_redraw("next_notification");
                    }
                },
                _ => println!("{}", cmd)
            };
            // app.request_redraw();
        }
        
        //println!("Ricevuto: {}", msg);
        if let Ok(data) = sock.rx.try_recv() {
            // println!("{} Ricevuto: {:?}", chrono::Local::now().format("%H:%M:%S%.3f"), data.resource);
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
                    // println!("{:?}", bat);
                    app.request_redraw("battery");
                    // eprintln!("{:?}", bat);
                    // eprintln!("battery {:?} {:?}", app.battery_recharging, app.battery_eta);
                }
            }

            // Bluetooth data has a custom management
            if data.resource == "bt-batteries" {
                eprintln!("{:?}", data);
                if data.warning >= 0.3 || config.show_always_bluetooth {
                    if let Some(blue) = &data.data {
                        if let Ok(b) = BluetoothStats::deserialize(blue.clone()) {
                            let keys: Vec<String> = app.icons
                                .keys()
                                .filter(|k| k.starts_with("bt-"))
                                .cloned()
                                .collect();
                            for iconkey in keys {
                                app.remove_icon(&iconkey);
                            }

                            for dev in b.devices {
                                // println!("device extracted: {:?}", dev);
                                let iconkey = format!("bt-{}", dev.name);
                                let icon = match dev.kind {
                                    DeviceKind::Mouse => "󰦋",
                                    DeviceKind::Headphones => "󰥰",
                                    DeviceKind::Gamepad => "󱤙",
                                    DeviceKind::Keyboard => "󰌌",
                                    _ => "󰂱"
                                };
                                if config.show_always_bluetooth || dev.warn >= 0.3 {
                                    app.add_icon(&iconkey, icon, get_color_gradient(dev.warn), dev.warn);
                                }
                            }
                            app.animator.animate_property(
                                &app.frame_model,
                                AnimationKey::IconsHeight,
                                app.icons.len() as f64,
                                200
                            );
                        }
                        // PartialMsg { resource: "bt-batteries", warning: 0.0, icon: "", data: Some(Object {"devices": Array [Object {"kind": String("Mouse"), "name": String("MX Anywhere 2S"), "percentage": Number(90.0), "warn": Number(0.0)}], "icon": String(""), "warn": Number(0.0)}) }
                    }
                }
            }

            if data.resource == "ratatoskr" {
                let new_ratatoskr_status = data.warning < 0.5;
                if app.ratatoskr_connected != new_ratatoskr_status {
                    app.ratatoskr_connected = new_ratatoskr_status;
                    app.request_redraw("ratatoskr");
                }
            } else if data.warning < 0.3 {
                if app.remove_icon(&data.resource) {
                    app.animator.animate_property(
                        &app.frame_model,
                        AnimationKey::IconsHeight,
                        app.icons.len() as f64,
                        200
                    );
                    app.request_redraw(&"data.resource");
                }
            }
            else {
                let mut icon = "";
                if data.resource == "loadavg" { icon = "󰬢"; }
                else if data.resource == "ram" { icon = "󰘚"; }
                else if data.resource == "temperature" { icon = &data.icon; }
                else if data.resource == "network" { icon = if data.icon != "" { &data.icon } else { "󰞃" }; }
                else if data.resource == "disk" { icon = "󰋊"; }
                else if data.resource == "volume" {
                    if let Some(vol) = &data.data {
                        if vol.get("headphones").unwrap().as_i64().unwrap() == 1 { icon = ""; }
                        else {
                            icon = select_icon(0.0, 100.0, vol["value"].as_f64().unwrap_or_default(), &["", "", ""]).unwrap();
                        }
                    } else {
                        icon = "󱄡";
                    }
                } // if data.icon != "" { &data.icon } else { "󱄡" }; }
                // weather
                // volume
                // disk
                // display

                if icon != "" {
                    let removed = app.remove_icon(&data.resource);
                    app.add_icon(&data.resource, icon, get_color_gradient(data.warning), data.warning);
                    if !removed {
                        app.animator.animate_property(
                            &app.frame_model,
                            AnimationKey::IconsHeight,
                            app.icons.len() as f64,
                            200
                        );
                    }
                    app.request_redraw(&data.resource);
                }
            }
        }

        if let Ok(list) = rx_notif.try_recv() {
            // draw_notifications(&list);
            // manage redraw
            println!("{:?}", list);
            if list.iter().any(|x| x.reboot) {
                app.add_icon("reboot", "󱄋", get_color_gradient(1.0), 1.0);
            }
            let changed = (app.notifications.len() == 0) != (list.len() == 0);
            if changed {
                app.animator.animate_property(
                    &app.frame_model,
                    AnimationKey::NotificationHeight,
                    if list.len() > 0 { 1.0 } else { 0.0 },
                    200
                );
                app.notifications = list;
            }
            app.request_redraw("notifications updated");
        }

        app.maybe_redraw(&qh);
        conn.flush().unwrap();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/*
self.animator.animate_property(
    self.alpha,
    1.0,
    Duration::from_millis(120),
    {
        let ptr = &mut self.alpha as *mut f32;
        move |v| unsafe {
            *ptr = v;
        }
    }
);
*/