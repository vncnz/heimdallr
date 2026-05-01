
// Copyright 2021 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0
// Copied and edited by vncnz

use std::sync::mpsc::Sender;
use colored::Colorize;
use futures::stream::StreamExt;

use zbus::{Connection, Proxy, dbus_proxy};

// use serde_repr::{Deserialize_repr, Serialize_repr};
// use zbus::zvariant::OwnedValue;

use crate::dbg_println;

// #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize_repr, Serialize_repr, OwnedValue)]
#[derive(Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum BatteryState {
    Unknown = 0,
    Charging = 1,
    Discharging = 2,
    Empty = 3,
    FullyCharged = 4,
    PendingCharge = 5,
    PendingDischarge = 6,
    NotCharging = 100 // Custom value, no dbus-defined
}

impl From<u32> for BatteryState {
    fn from(value: u32) -> Self {
        match value {
            0 => BatteryState::Unknown,
            1 => BatteryState::Charging,
            2 => BatteryState::Discharging,
            3 => BatteryState::Empty,
            4 => BatteryState::FullyCharged,
            5 => BatteryState::PendingCharge,
            6 => BatteryState::PendingDischarge,
            100 => BatteryState::NotCharging,
            _ => BatteryState::Unknown,
        }
    }
}

/* #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize_repr, Serialize_repr, OwnedValue)]
#[repr(u32)]
pub enum BatteryType {
    Unknown = 0,
    LinePower = 1,
    Battery = 2,
    Ups = 3,
    Monitor = 4,
    Mouse = 5,
    Keyboard = 6,
    Pda = 7,
    Phone = 8,
} */

/* #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize_repr, Serialize_repr, OwnedValue)]
#[repr(u32)]
pub enum BatteryLevel {
    Unknown = 0,
    None = 1,
    Low = 3,
    Critical = 4,
    Normal = 6,
    High = 7,
    Full = 8,
} */

#[derive(Debug, Clone, PartialEq)]
pub struct BatteryStats {
    pub state: BatteryState,
    pub percentage: f64,
    pub eta_minutes: Option<f64>,
}
/*
#[dbus_proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPPower",
    assume_defaults = false
)]
trait Device {
    #[dbus_proxy(property)]
    fn battery_level(&self) -> zbus::Result<BatteryLevel>;

    #[dbus_proxy(property)]
    fn capacity(&self) -> zbus::Result<f64>;

    #[dbus_proxy(property)]
    fn energy(&self) -> zbus::Result<f64>;

    #[dbus_proxy(property)]
    fn energy_empty(&self) -> zbus::Result<f64>;

    #[dbus_proxy(property)]
    fn energy_full(&self) -> zbus::Result<f64>;

    #[dbus_proxy(property)]
    fn energy_full_design(&self) -> zbus::Result<f64>;

    #[dbus_proxy(property)]
    fn has_history(&self) -> zbus::Result<bool>;

    #[dbus_proxy(property)]
    fn has_statistics(&self) -> zbus::Result<bool>;

    #[dbus_proxy(property)]
    fn icon_name(&self) -> zbus::Result<String>;

    #[dbus_proxy(property)]
    fn is_present(&self) -> zbus::Result<bool>;

    #[dbus_proxy(property)]
    fn is_rechargeable(&self) -> zbus::Result<bool>;

    #[dbus_proxy(property)]
    fn luminosity(&self) -> zbus::Result<f64>;

    #[dbus_proxy(property)]
    fn model(&self) -> zbus::Result<String>;

    #[dbus_proxy(property)]
    fn native_path(&self) -> zbus::Result<String>;

    #[dbus_proxy(property)]
    fn online(&self) -> zbus::Result<bool>;

    #[dbus_proxy(property)]
    fn percentage(&self) -> zbus::Result<f64>;

    #[dbus_proxy(property)]
    fn power_supply(&self) -> zbus::Result<bool>;

    fn refresh(&self) -> zbus::Result<()>;

    #[dbus_proxy(property)]
    fn serial(&self) -> zbus::Result<String>;

    #[dbus_proxy(property)]
    fn state(&self) -> zbus::Result<BatteryState>;

    #[dbus_proxy(property)]
    fn temperature(&self) -> zbus::Result<f64>;

    #[dbus_proxy(property, name = "Type")]
    fn type_(&self) -> zbus::Result<BatteryType>;

    #[dbus_proxy(property)]
    fn vendor(&self) -> zbus::Result<String>;

    #[dbus_proxy(property)]
    fn voltage(&self) -> zbus::Result<f64>;
}

#[dbus_proxy(interface = "org.freedesktop.UPower", assume_defaults = true)]
trait UPower {
    /// EnumerateDevices method
    fn enumerate_devices(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;

    /// GetCriticalAction method
    fn get_critical_action(&self) -> zbus::Result<String>;

    /// GetDisplayDevice method
    #[dbus_proxy(object = "Device")]
    fn get_display_device(&self);

    /// DeviceAdded signal
    #[dbus_proxy(signal)]
    fn device_added(&self, device: zbus::zvariant::ObjectPath<'_>) -> zbus::Result<()>;

    /// DeviceRemoved signal
    #[dbus_proxy(signal)]
    fn device_removed(&self, device: zbus::zvariant::ObjectPath<'_>) -> zbus::Result<()>;

    /// DaemonVersion property
    #[dbus_proxy(property)]
    fn daemon_version(&self) -> zbus::Result<String>;

    /// LidIsClosed property
    #[dbus_proxy(property)]
    fn lid_is_closed(&self) -> zbus::Result<bool>;

    /// LidIsPresent property
    #[dbus_proxy(property)]
    fn lid_is_present(&self) -> zbus::Result<bool>;

    /// OnBattery property
    #[dbus_proxy(property)]
    fn on_battery(&self) -> zbus::Result<bool>;
}
*/
pub async fn start_battery_listener_events(tx: Sender<BatteryStats>) -> zbus::Result<()> {
    let connection = Connection::system().await?;
    
    let signal_proxy = Proxy::new(
        &connection,
        "org.freedesktop.UPower",
        "/org/freedesktop/UPower/devices/DisplayDevice",
        "org.freedesktop.DBus.Properties",
    ).await?;

    let device_proxy = Proxy::new(
        &connection,
        "org.freedesktop.UPower",
        "/org/freedesktop/UPower/devices/DisplayDevice",
        "org.freedesktop.UPower.Device",
    ).await?;

    let get_stats = async |p: &Proxy| -> zbus::Result<BatteryStats> {
        let state_val: u32 = p.get_property("State").await?;
        // eprintln!("{state_val}");
        let state = BatteryState::from(state_val);
        let percentage: f64 = p.get_property("Percentage").await?;
        let eta_seconds: i64 = match state {
            BatteryState::Charging | BatteryState::PendingCharge => { p.get_property("TimeToFull").await? },
            BatteryState::Discharging | BatteryState::PendingDischarge => { p.get_property("TimeToEmpty").await? },
            _ => { 0 }
        };
        // let time_to_empty: i64 = p.get_property("TimeToEmpty").await?;
        
        let obj = BatteryStats {
            state,
            percentage,
            eta_minutes: if eta_seconds > 0 { Some((eta_seconds as f64) / 60.0) } else { None }
        };
        // dbg_println!("{obj:?}");
        Ok(obj)
    };

    if let Ok(stats) = get_stats(&device_proxy).await {
        dbg_println!("{}", "Battery first info load!".yellow());
        let _ = tx.send(stats);
    }

    let mut signal_iterator = signal_proxy.receive_signal("PropertiesChanged").await?;

    let tx = tx.clone();
    std::thread::spawn(move || {
        futures::executor::block_on(async {
    
            while let Some(_) = signal_iterator.next().await {
                dbg_println!("{}", "Battery signal!".yellow());

                let res = get_stats(&device_proxy).await;
                match res {
                    Ok(stats) => {
                        dbg_println!("{}", "Sending battery signal!".yellow());
                        let _ = tx.send(stats);
                    },
                    Err(err) => {
                        eprintln!("{err:?}");
                    }
                }
            }
        });
    });

    Ok(())
}
/*
pub async fn start_battery_listener_poll(tx: Sender<BatteryStats>) -> zbus::Result<()> {
    let connection = zbus::Connection::system().await?;

    let upower = UPowerProxy::new(&connection).await?;

    // Spawn a task that polls for battery changes and sends updates via tx
    let tx = tx.clone();
    std::thread::spawn(move || {
        futures::executor::block_on(async {
            // Get initial state
            let mut last_state: Option<BatteryStats> = None;
            
            loop {
                std::thread::sleep(Duration::from_secs(5));
                
                // Fetch current battery stats
                let current = match get_battery_stats(&upower).await {
                    Ok(stats) => stats,
                    Err(e) => {
                        eprintln!("Error reading battery: {}", e);
                        continue;
                    }
                };
                
                // Only send if state changed
                if last_state != Some(current.clone()) {
                    last_state = Some(current.clone());
                    let _ = tx.send(current);
                } else {
                    let _ = tx.send(current);
                }
            }
        });
    });

    Ok(())
} */
/*
async fn get_battery_stats(upower: &UPowerProxy<'_>) -> zbus::Result<BatteryStats> {
    // Use the well-known DisplayDevice path directly
    let path_str = "/org/freedesktop/UPower/devices/DisplayDevice";
    
    // Create a proxy for the device using the raw Proxy API
    let device_proxy = zbus::Proxy::new(
        upower.connection(),
        "org.freedesktop.UPower",
        path_str,
        "org.freedesktop.UPower.Device",
    ).await?;
    
    // Debug: print raw property values
    let state_val: u32 = device_proxy.get_property("State").await.unwrap_or(0);
    let percentage: f64 = device_proxy.get_property("Percentage").await.unwrap_or(0.0);
    let time_to_empty: i64 = device_proxy.get_property("TimeToEmpty").await.unwrap_or(0);
    
    eprintln!("DEBUG battery: State={}, Percentage={}, TimeToEmpty={}", state_val, percentage, time_to_empty);
    
    Ok(BatteryStats {
        state: BatteryState::from(state_val),
        percentage,
        eta_minutes: (time_to_empty as f64) / 60.0,
    })
}
*/

pub async fn start_battery_listener_dbus_mix_polling_and_events(tx: Sender<BatteryStats>) -> zbus::Result<()> {
    let connection = Connection::system().await?;
    
    let signal_proxy = Proxy::new(
        &connection,
        "org.freedesktop.UPower",
        "/org/freedesktop/UPower/devices/DisplayDevice",
        "org.freedesktop.DBus.Properties",
    ).await?;

    let device_proxy = Proxy::new(
        &connection,
        "org.freedesktop.UPower",
        "/org/freedesktop/UPower/devices/DisplayDevice",
        "org.freedesktop.UPower.Device",
    ).await?;

    let get_stats = async |p: &Proxy| -> zbus::Result<BatteryStats> {
        dbg_println!("{}", "get_stats started".blue());
        let state_val: u32 = p.get_property("State").await?;
        let state = BatteryState::from(state_val);
        let percentage: f64 = p.get_property("Percentage").await?;
        let eta_seconds: i64 = match state {
            BatteryState::Charging | BatteryState::PendingCharge => { p.get_property("TimeToFull").await? },
            BatteryState::Discharging | BatteryState::PendingDischarge => { p.get_property("TimeToEmpty").await? },
            _ => { 0 }
        };
        
        let obj = BatteryStats {
            state,
            percentage,
            eta_minutes: if eta_seconds > 0 { Some((eta_seconds as f64) / 60.0) } else { None }
        };
        dbg_println!("{} {}", "get_stats ended".blue(), eta_seconds);
        Ok(obj)
    };

    if let Ok(stats) = get_stats(&device_proxy).await {
        dbg_println!("{}", "Battery first info load!".yellow());
        let _ = tx.send(stats);
    }

    let mut signal_iterator = signal_proxy.receive_signal("PropertiesChanged").await?;

    let tx_signal = tx.clone();
    let device_proxy_signal = device_proxy.clone();
    
    // Thread per i segnali DBus (quando lo stato cambia)
    std::thread::spawn(move || {
        futures::executor::block_on(async {
            while let Some(_) = signal_iterator.next().await {
                dbg_println!("{}", "Battery signal!".yellow());

                let res = get_stats(&device_proxy_signal).await;
                match res {
                    Ok(stats) => {
                        dbg_println!("{}", "Sending battery signal!".yellow());
                        let _ = tx_signal.send(stats);
                    },
                    Err(err) => {
                        eprintln!("{err:?}");
                    }
                }
            }
        });
    });

    // Thread per il polling (solo quando si sta caricando/scaricando)
    let tx_poll = tx.clone();
    let device_proxy_poll = device_proxy.clone();
    
    std::thread::spawn(move || {
        futures::executor::block_on(async {
            let mut last_stats: Option<BatteryStats> = None;
            let poll_interval = std::time::Duration::from_secs(2);
            
            loop {
                std::thread::sleep(poll_interval);
                
                if let Ok(stats) = get_stats(&device_proxy_poll).await {
                    // Poll if charging/discharging
                    let should_poll = matches!(
                        stats.state,
                        BatteryState::Charging | BatteryState::Discharging |
                        BatteryState::PendingCharge | BatteryState::PendingDischarge
                    );
                    
                    if should_poll {
                        // Send if eta changed
                        let eta_changed = last_stats.as_ref().map_or(true, |last| {
                            last.eta_minutes != stats.eta_minutes
                        });
                        
                        if eta_changed {
                            dbg_println!("{}", "Sending battery update (polling)!".yellow());
                            let _ = tx_poll.send(stats.clone());
                            last_stats = Some(stats);
                        }
                    } else if last_stats.is_some() {
                        // Se eravamo in polling e ora lo stato è inattivo, invia un ultimo update
                        let _ = tx_poll.send(stats.clone());
                        last_stats = None;
                    }
                }
            }
        });
    });

    Ok(())
}




























use std::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

pub struct SysBatteryReader {
    buffer: String,
    path: String,
}

impl SysBatteryReader {
    pub fn new(bat_name: &str) -> Self {
        Self {
            buffer: String::with_capacity(64),
            path: format!("/sys/class/power_supply/{}/", bat_name),
        }
    }

    fn read_val(&mut self, file_name: &str) -> f64 {
        self.buffer.clear();
        if let Ok(mut f) = File::open(format!("{}{}", self.path, file_name)) {
            let _ = f.read_to_string(&mut self.buffer);
            self.buffer.trim().parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    pub fn get_stats(&mut self) -> BatteryStats {
        let state = self.get_battery_state();
        let percentage = self.read_val("capacity");

        if state == BatteryState::Charging || state == BatteryState::Discharging {
            let energy = self.read_val("energy_now");
            let power = self.read_val("power_now");
            let full = self.read_val("energy_full");

            let eta_minutes = if state == BatteryState::Discharging && power > 0.0 {
                Some((energy / power) * 60.0)
            } else if state == BatteryState::Charging && power > 0.0 {
                Some(((full - energy) / power) * 60.0)
            } else {
                None
            };
            return BatteryStats { state, percentage, eta_minutes };
        } else {
            BatteryStats { state, percentage, eta_minutes: None }
        }
    }

    pub fn get_battery_state(&mut self) -> BatteryState {
        let status_str = fs::read_to_string(format!("{}/status", self.path))
            .unwrap_or_else(|_| "Unknown".to_string());

        match status_str.trim() {
            "Charging" => BatteryState::Charging,
            "Discharging" => BatteryState::Discharging,
            "Not charging" => BatteryState::NotCharging,
            "Full" => BatteryState::FullyCharged,
            _ => BatteryState::Unknown,
        }
    }
}

pub async fn start_battery_listener(tx: Sender<BatteryStats>) -> zbus::Result<()> {
    let mut bat = SysBatteryReader::new("BAT0");
    std::thread::spawn(move || {
        futures::executor::block_on(async {
            let poll_interval = std::time::Duration::from_secs(2);
            
            loop {
                let new_stats = bat.get_stats();
                dbg_println!("{} {:?}", "Sending battery signal!".blue(), new_stats);
                let _ = tx.send(new_stats);
                std::thread::sleep(poll_interval);
            }
        });
    });
    Ok(())
}