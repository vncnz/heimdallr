use signal_hook::{consts::{SIGHUP, SIGINT, SIGPIPE, SIGTERM}, iterator::Signals, low_level::signal_name};

use serde::Deserialize;
use smithay_client_toolkit::{
    compositor::CompositorState, output::OutputState, registry::RegistryState, shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell}, shm::Shm
};
use wayland_client::{Connection, EventQueue, globals::{GlobalList, registry_queue_init}, protocol::{wl_compositor, wl_output::WlOutput, wl_region}};

use std::{sync::mpsc::{self, Receiver, Sender}, time::{Duration}};

use smithay_client_toolkit::shell::WaylandSurface;

use std::panic;
use std::thread;

use colored::Colorize;

use crate::{battery::{BatteryState, BatteryStats}, commands::start_command_listener, data::{BluetoothStats, IconChange, RatatoskrSocket}, notifications::Notification, security::{MicCameraStatus, start_security_monitor}, utils::{get_color_gradient, log_to_file, select_icon}};

mod data;
mod config;
// mod heimdallr_layer;
// mod heimdallr_layer_old;
//mod clock;
//mod clock1;
//mod clock2;
mod heimdallr_layer;
mod notifications;
mod commands;
mod utils;
mod battery;
mod security;
mod countdown;
mod pills;

use config::Config;
// use chrono;

use crate::heimdallr_layer::HeimdallrLayer;
use crate::notifications::start_notification_listener;
use crate::battery::start_battery_listener;

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

    let mut app = HeimdallrLayer::new(
        RegistryState::new(&globals),
        OutputState::new(&globals, &qh),
        Shm::bind(&globals, &qh).unwrap(),
        config.clone()
    );

    if !config.hide_missing_ratatoskr {
        app.add_icon("ratatoskr", "󰠗", get_color_gradient(1.0), 1.0, None);
        /* app.animator.animate_property(
            &app.frame_model,
            AnimationKey::IconsHeight,
            app.icons.len() as f64,
            200
        ); */
    }

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
    
    thread::spawn(|| {
        futures::executor::block_on(async {
            if let Err(e) = start_notification_listener(tx).await {
                log_to_file(format!("Notification listener error: {:?}", e));
                let msg = format!("Notification listener error: {:?}", e).red().to_string();
                dbg_println!("{}", msg);
            } else {
                dbg_println!("{}", "Notification listener OK".green().to_string());
            }
        });
    });

    let (tx_battery, rx_battery): (Sender<BatteryStats>, Receiver<BatteryStats>) = mpsc::channel();
    thread::spawn(|| {
        start_battery_listener(tx_battery);
        dbg_println!("{}", "Battery listener OK".green().to_string());
    });

    let (tx_pipewire, rx_pipewire): (Sender<MicCameraStatus>, Receiver<MicCameraStatus>) = mpsc::channel();
    thread::spawn(|| {
        futures::executor::block_on(async {
            if let Err(e) = start_security_monitor(tx_pipewire) {
                log_to_file(format!("PipeWire listener error: {:?}", e));
                dbg_println!("{}", format!("PipeWire listener error: {:?}", e).red().to_string());
            } else {
                dbg_println!("{}", "PipeWire listener OK".green().to_string());
            }
        });
    });

    let (demo_tx, demo_rx) = mpsc::channel::<(String, String)>();


    if false {
        thread::spawn(move || {
            let actions = vec![
                ("warning-ram", "0.9", Duration::from_secs(1)),
                //("warning-load", "0.6", Duration::from_secs(1)),
                //("warning-disk", "0.31", Duration::from_secs(1))
                /*
                ("battery", "false", Duration::from_secs(5)),
                ("battery", "true", Duration::from_secs(2)),
                ("security", "on", Duration::from_secs(4)),
                ("security", "off", Duration::from_secs(2)),
                ("wob", "0.35", Duration::from_secs(1)),
                ("wob", "0.45", Duration::from_millis(200)),
                ("wob", "0.55", Duration::from_millis(300)),
                ("wob", "0.65", Duration::from_millis(200)),
                ("timer", "3s", Duration::from_secs(3)),
                ("timer", "off", Duration::from_secs(6)),
                ("notification", "The endless noise will put a lock on your open mind, and it will tear your soul apart. Run away from the tragedy, and find the essence of silence that remains inside.", Duration::from_secs(2)) */
            ];

            for (kind, value, delay) in actions {
                std::thread::sleep(delay);
                let _ = demo_tx.send((kind.to_string(), value.to_string()));
            }
        });
    }


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

        // Dispatch demo events
        if let Ok((kind, value)) = demo_rx.try_recv() {
            match (kind.as_str(), value.as_str()) {
                ("timer", time) => {
                    app.set_countdown(&time).ok();
                    app.request_redraw("demo timer");
                },
                ("security", "on") => {
                    app.update_security_data(MicCameraStatus { mic_active: vec!["Firefox".to_string()], camera_active: vec![], pristine: true });
                    app.request_redraw("demo security on");
                },
                ("security", "off") => {
                    app.update_security_data(MicCameraStatus { mic_active: vec!(), camera_active: vec!(), pristine: true });
                    app.request_redraw("demo security off");
                },
                ("wob", val) => {
                    if let Ok(value) = val.parse::<f64>() {
                        app.show_value(value, None);
                    } else {
                        eprintln!("Invalid wob value: {}", val);
                    }
                },
                ("notification", text) => {
                    let notif = Notification {
                        app_name: "Demo notification".to_string(),
                        summary: "Demo notification".to_string(),
                        body: text.to_string(),
                        urgency: 1,
                        received_at: std::time::Instant::now(),
                        expired_at: Some(std::time::Instant::now() + Duration::from_secs(3)),
                        app_icon: "dialog-information".to_string(),
                        id: 0,
                        replaces_id: 0,
                        unmounting: false,
                        unmounted: false,
                        reboot: false,
                        datetime: chrono::Local::now()
                    };
                    let _ = app.update_notification_list(Some(notif));
                    app.request_redraw("demo notification");
                },
                ("battery", charging_str) => {
                    if let Ok(charging) = charging_str.parse::<bool>() {
                        let bat = BatteryStats {
                            percentage: 60.0,
                            state: if charging { BatteryState::Charging } else { BatteryState::Discharging },
                            eta_minutes: Some(if charging { 12.0 } else { 312.0 }),
                            flow: Some(10.34)
                        };
                        app.update_battery_data(Some(bat));
                        app.request_redraw("demo battery");
                    }
                },
                ("warning-ram", w) => {
                    if let Ok(w) = w.parse::<f64>() {
                        if w > 0.3 { app.add_icon("demoram", "󰘚", get_color_gradient(w), w, None); }
                        else { app.remove_icon("demoram"); }
                    }
                },
                ("warning-load", w) => {
                    if let Ok(w) = w.parse::<f64>() {
                        if w > 0.3 { app.add_icon("demoload", "󰬢", get_color_gradient(w), w, None); }
                        else { app.remove_icon("demoload"); }
                    }
                },
                ("warning-disk", w) => {
                    if let Ok(w) = w.parse::<f64>() {
                        if w > 0.3 { app.add_icon("demodisk", "󰋊", get_color_gradient(w), w, None); }
                        else { app.remove_icon("demodisk"); }
                    }
                },
                // "󰞃"
                _ => {
                    eprintln!("Unknown demo command: {} {}", kind, value);
                }
            }
        }

        if let Ok(bat) = rx_battery.try_recv() {
            app.update_battery_data(Some(bat));
            app.request_redraw(&"battery");
            eprintln!("{}", "Battery update".yellow());
        }

        /* if is_camera_in_use() {
            eprintln!("{}", "Camera in use!".red());
        } */

        if let Ok(status) = rx_pipewire.try_recv() {
            // app.mic_active = status.mic_active;
            // app.camera_active = status.camera_active;
            // app.request_redraw(&"pipewire");
            // eprintln!("{}", "PipeWire update".bright_blue());
            log_to_file(format!("{:?}", status).to_string());
            println!("{}", format!("{:?}", status).red());
            app.update_security_data(status);
            app.request_redraw("security updated"); // TODO: in the new system, pill will know if it needs redraw, without forcing here
        }

        if let Ok(cmd) = rx_cmds.try_recv() {
            match &*cmd {
                "hide_notification" => {
                    println!("hide!");
                    if app.remove_notification() {
                        app.request_redraw("hide_notification");
                    } else {
                        eprintln!("--- No remove?");
                    }
                },
                /* "prev_notification" => {
                    if app.show_notification(-1) {
                        app.request_redraw("prev_notification");
                    }
                }, */
                /* "next_notification" => {
                    if app.show_notification(1) {
                        app.request_redraw("next_notification");
                    }
                }, */
                _ => {
                    println!("cmd to be parsed: {}", cmd);
                    let parts: Vec<&str> = cmd.split(" ").collect();
                    match parts.as_slice() {
                        ["timer", value_str] => {
                            match app.set_countdown(value_str) {
                                Ok(secs) => {
                                    if secs > 0 {
                                        // app.update_timer_icon();
                                    } else {
                                        app.remove_icon("timer");
                                    }
                                    app.request_redraw("timer set");
                                    eprintln!("Timer set to {} seconds", secs);
                                },
                                Err(err) => {
                                    eprintln!("Error setting timer: {err}");
                                }
                            }
                            /* match value_str.parse::<f64>() {
                                Ok(value) => { eprintln!("Timer set to {} seconds", value); true },
                                Err(_) => { eprintln!("Invalid number: {}", value_str); false }
                            } */
                        }
                        
                        [kind, value_str] => {
                            match value_str.parse::<f64>() {
                                Ok(value) => { app.show_value(value, Some(*kind)); },
                                Err(_) => { eprintln!("Invalid number: {}", value_str); }
                            }
                        }

                        [value_str] => {
                            match value_str.parse::<f64>() {
                                Ok(value) => { app.show_value(value, None); },
                                Err(_) => { eprintln!("Invalid number: {}", value_str); }
                            }
                        }

                        _ => {
                            eprintln!("Unknown command");
                        }
                    };
                }
            };
            // app.request_redraw();
        }
        
        //println!("Ricevuto: {}", msg);
        if let Ok(data) = sock.rx.try_recv() {
            // println!("{} Ricevuto: {:?}", chrono::Local::now().format("%H:%M:%S%.3f"), data.resource);
            if data.resource == "battery" {
                // Now I'm trying to get battery infos internally!
                /* if let Some(bat) = &data.data {
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
                } */
            }

            // Bluetooth data has a custom management
            if data.resource == "bt-batteries" {
                // dbg_println!("{:?}", data);
                log_to_file(format!("{:?}", data));
                if let Some(blue) = &data.data {
                    if let Ok(b) = BluetoothStats::deserialize(blue.clone()) {
                        /* if config.show_always_bluetooth {
                            let keys: Vec<String> = app.icons
                                .keys()
                                .filter(|k| k.starts_with("bt-"))
                                .cloned()
                                .collect();
                            for iconkey in keys {
                                app.remove_icon(&iconkey);
                            }

                            for dev in b.devices.clone().iter().filter(|dv| dv.is_bluetooth) {
                                // println!("device extracted: {:?}", dev);
                                let iconkey = format!("bt-{}", dev.name);
                                let icon = match dev.kind {
                                    UPowerDeviceKind::Mouse => "󰦋",
                                    UPowerDeviceKind::Phone => "󱆏",
                                    UPowerDeviceKind::Tablet => "",
                                    UPowerDeviceKind::RemoteControl => "󰻅",
                                    UPowerDeviceKind::Speakers => "󰦢",
                                    UPowerDeviceKind::Headphones => "󰥰",
                                    UPowerDeviceKind::GamingInput => "󱤙",
                                    UPowerDeviceKind::Keyboard => "󰌌",
                                    _ => "󰂱"
                                };
                            }
                        } */
                        if app.batteries != b.devices {
                            app.update_devices_data(b.devices);
                            app.request_redraw("bt-batteries");
                        } else {
                            dbg_println!("{}", format!("Bluetooth battery status unchanged").yellow());
                        }
                        // PartialMsg { resource: "bt-batteries", warning: 0.0, icon: "", data: Some(Object {"devices": Array [Object {"kind": String("Mouse"), "name": String("MX Anywhere 2S"), "percentage": Number(90.0), "warn": Number(0.0)}], "icon": String(""), "warn": Number(0.0)}) }
                    }
                }
            }

            if data.resource == "ratatoskr" {
                let new_ratatoskr_status = data.warning < 0.5;
                if app.ratatoskr_connected != new_ratatoskr_status {
                    app.ratatoskr_connected = new_ratatoskr_status;
                    if !new_ratatoskr_status {
                        app.icons.clear();
                        app.update_devices_data(Vec::new());
                        /* let keys: Vec<String> = app.icons.keys().cloned().collect();
                        for iconkey in keys {
                            app.remove_icon(&iconkey);
                        } */
                        if !config.hide_missing_ratatoskr { app.add_icon("ratatoskr", "󰠗", get_color_gradient(1.0), 1.0, None); }
                    } else {
                        app.remove_icon("ratatoskr");
                    }
                    app.request_redraw("ratatoskr");
                }
            } else if data.warning < 0.3 {
                if app.remove_icon(&data.resource) {
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
                            let slice: &[&str] = &["", "", ""].as_slice();
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
                    let change = app.add_icon(&data.resource, icon, get_color_gradient(data.warning), data.warning, None);
                    
                    if change != IconChange::None {
                        if change == IconChange::Added {
                            dbg_println!("Icon added");
                        } else {
                            dbg_println!("Icon changed");
                        }
                        app.request_redraw(&data.resource);
                    } else {
                        // dbg_println!("Icon untouched {} {}", data.resource, data.warning);
                    }
                }
            }
        }

        if let Ok(new_notif) = rx_notif.try_recv() {
            println!("{:?}", new_notif);
            if new_notif.reboot {
                app.add_icon("reboot", "󱄋", get_color_gradient(1.0), 1.0, None);
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