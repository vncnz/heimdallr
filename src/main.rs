use signal_hook::{consts::{SIGHUP, SIGINT, SIGPIPE, SIGTERM}, iterator::Signals, low_level::signal_name};

use serde::Deserialize;
use smithay_client_toolkit::{
    compositor::CompositorState, output::OutputState, registry::RegistryState, shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell}, shm::Shm
};
use wayland_client::{Connection, EventQueue, globals::{GlobalList, registry_queue_init}, protocol::{wl_compositor, wl_output::WlOutput, wl_region}};

use std::{sync::mpsc::{self, Receiver, Sender}, time::{Duration, Instant}};

use smithay_client_toolkit::shell::WaylandSurface;

use std::collections::HashMap;

use std::panic;

use crate::{clock::{ClockTrait, NoClock}, clock1::Clock1, commands::start_command_listener, data::{BluetoothStats, DeviceKind, RatatoskrSocket}, heimdallr_layer::IconChange, notifications::Notification, utils::{AnimationKey, Animator, FrameModel, get_color_gradient, log_to_file, select_icon}};

mod data;
mod config;
mod heimdallr_layer;
mod notifications;
mod commands;
mod utils;
mod clock;
mod clock1;
use config::Config;
// use chrono;

use crate::heimdallr_layer::HeimdallrLayer;
use crate::notifications::start_notification_listener;

use clap::{crate_name, crate_version, Parser};

#[derive(Debug, Parser)]
#[command(disable_version_flag = true, about = "Zero-config system HUD for Wayland", long_about = None)]
struct Args {
    /* /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>, */

    //#[arg(short, long, default_value_t = 1)]
    //count: u8,

    #[arg(short = 'V', long, help = "Print version")]
    version: bool,
}

fn choose_output (app: &HeimdallrLayer) -> std::option::Option<WlOutput>{
    let mut chosen_output = None;
    for output in app.output_state.outputs() {
        if let Some(info) = app.output_state.info(&output) {
            // eprintln!("Display info {:?}", info);
            if let Some(name) = info.name {
                log_to_file(format!("Found display {name}"));
                if name.starts_with("eDP") {
                    chosen_output = Some(output.clone());
                    // dbg_println!("Found internal display");
                    log_to_file(format!("{name} is an embedded display!"));
                }
            }
        }
        if chosen_output.is_none() {
            chosen_output = Some(output.clone());
        }
    }
    chosen_output
}

fn main() {

    let args = Args::parse();

    if args.version {
        println!("{} {}", crate_name!(), crate_version!());
        std::process::exit(0);
    }

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
            log_to_file(format!("Received signal {sig} ({name}) from the system"));

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

    log_to_file(format!("{} {} started", crate_name!(), crate_version!()));

    let config = Config::load_from_file("~/.config/heimdallr/config.json");
    log_to_file(format!("Loaded configuration: {:?}", config));

    let conn = Connection::connect_to_env().unwrap();
    let (globals, mut event_queue): (GlobalList, EventQueue<HeimdallrLayer>) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).unwrap();
    let layer_shell = LayerShell::bind(&globals, &qh).unwrap();
    let shm = Shm::bind(&globals, &qh).unwrap();

    //let resources = Arc::new(Mutex::new(ResourceData::default()));
    //let receiver = start_resource_watcher("/tmp/ratatoskr.json", resources.clone());

    // let rx = data::start_socket_watcher("/tmp/ratatoskr.sock");

    let mut app = HeimdallrLayer {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool: None,
        layer: None,
        width: 1,
        height: 1,
        first_configure: true,
        // input_region: Some(empty_region),
        icons: HashMap::new(),
        ratatoskr_connected: false,
        battery_eta: None,
        battery_recharging: None,
        needs_redraw: true,
        last_redraw: Instant::now(),
        redraw_interval: [Duration::from_millis(1_000), Duration::from_millis(60_000)],
        buffers: [None, None],
        current_buffer_idx: 0,
        config: config.clone(),
        notifications: vec![],
        notification_idx: 0,
        wob_expiration: None,
        wob_value: 0.35, // TODO: set 0
        animator: Animator::new(),
        frame_model: FrameModel::new(),
        is_waiting_for_frame: false,
        clock: if config.show_clock {
            Box::new(Clock1::new()) as Box<dyn ClockTrait>
        } else {
            Box::new(NoClock::new()) as Box<dyn ClockTrait>
        }
    };

    event_queue.roundtrip(&mut app).unwrap();
    // event_queue.dispatch_pending(&mut app).unwrap();
    // let chosen_output = choose_output(&globals, &qh);
    // let output_state = OutputState::new(&globals, &qh);
    // let mut outputs = output_state.outputs();
    // let chosen_output = outputs.next();
    let chosen_output = choose_output(&app);

    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some("heimdallr"), chosen_output.as_ref());
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);

    let raw_compositor: wl_compositor::WlCompositor =
    globals.bind::<wl_compositor::WlCompositor, _, _>(&qh, 1..=4, ())
    .expect("failed to bind wl_compositor for region creation");

    let empty_region: wl_region::WlRegion = raw_compositor.create_region(&qh, ());
    layer.wl_surface().set_input_region(Some(&empty_region));

    layer.set_size(0, 0); // full screen
    layer.commit();

    app.layer = Some(layer);
    
    // app.add_icon("avg", "󰬢", (1.0, 0.2, 0.2, 1.0)); // example
    let (tx, rx_cmds): (Sender<String>, Receiver<String>) = mpsc::channel();
    let _ = start_command_listener(tx, "/tmp/heimdallr_cmds");

    let mut sock = RatatoskrSocket::new("/tmp/ratatoskr.sock");

    let (tx, rx_notif): (Sender<Notification>, Receiver<Notification>) = mpsc::channel();
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
                if let Err(_) = guard.read() {
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
                        app.animator.animate_property(
                            &app.frame_model,
                            AnimationKey::NotificationHeight,
                            if app.notifications.len() > 0 { 1.0 } else { 0.0 },
                            200
                        );
                        app.request_redraw("hide_notification");
                    } else {
                        eprintln!("--- No remove?");
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
                _ => {
                    println!("{}", cmd);
                    let parts: Vec<&str> = cmd.split(" ").collect();
                    let refresh = match parts.as_slice() {
                        [kind, value_str] => {
                            match value_str.parse::<f64>() {
                                Ok(value) => app.show_value(value, Some(*kind)),
                                Err(_) => { eprintln!("Invalid number: {}", value_str); false }
                            }
                        }

                        [value_str] => {
                            match value_str.parse::<f64>() {
                                Ok(value) => app.show_value(value, None),
                                Err(_) => { eprintln!("Invalid number: {}", value_str); false }
                            }
                        }

                        _ => {
                            eprintln!("Unknown command");
                            false
                        }
                    };
                    if refresh {
                        app.animator.animate_property(
                            &app.frame_model,
                            AnimationKey::WobHeightRatio,
                            1.0,
                            500
                        );
                        app.request_redraw("external value event");
                    }
                }
            };
            // app.request_redraw();
        }
        
        //println!("Ricevuto: {}", msg);
        if let Ok(data) = sock.rx.try_recv() {
            // println!("{} Ricevuto: {:?}", chrono::Local::now().format("%H:%M:%S%.3f"), data.resource);
            if data.resource == "battery" {
                if let Some(bat) = &data.data {
                    let battery_eta = app.battery_eta;
                    let battery_recharging = app.battery_recharging;
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
                    if battery_eta != app.battery_eta || battery_recharging != app.battery_recharging {
                        app.request_redraw("battery");
                    }
                    // dbg_println!("{:?}", bat);
                    // dbg_println!("battery {:?} {:?}", app.battery_recharging, app.battery_eta);
                }
            }

            // Bluetooth data has a custom management
            if data.resource == "bt-batteries" {
                // dbg_println!("{:?}", data);
                log_to_file(format!("{:?}", data));
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
                                    let _added = app.add_icon(&iconkey, icon, get_color_gradient(dev.warn), dev.warn);
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
                            let slice: &[&str] = &["", "", ""][..];
                            icon = select_icon(0.0, 100.0, vol["value"].as_f64().unwrap_or_default(), slice).unwrap();
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
                    // let removed = app.remove_icon(&data.resource);
                    let change = app.add_icon(&data.resource, icon, get_color_gradient(data.warning), data.warning);
                    
                    if change != IconChange::None {
                        if change == IconChange::Added {
                            dbg_println!("Icon added");
                            app.animator.animate_property(
                                &app.frame_model,
                                AnimationKey::IconsHeight,
                                app.icons.len() as f64,
                                200
                            );
                        } else {
                            dbg_println!("Icon changed");
                        }
                        app.request_redraw(&data.resource);
                    } else {
                        dbg_println!("Icon untouched {} {}", data.resource, data.warning);
                    }
                }
            }
        }

        if let Ok(new_notif) = rx_notif.try_recv() {
            println!("{:?}", new_notif);
            if new_notif.reboot {
                app.add_icon("reboot", "󱄋", get_color_gradient(1.0), 1.0);
            }
            app.update_notification_list(Some(new_notif));
            app.request_redraw("notifications updated");
        }

        app.check_redraw_timeout();
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