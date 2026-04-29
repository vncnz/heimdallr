use zbus::{Connection, Proxy};
use std::sync::mpsc::Sender;
use futures::StreamExt;
use colored::Colorize;

use crate::dbg_println;

#[derive(Debug, Clone)]
pub struct BatteryStats {
    pub state: BatteryState,
    pub percentage: f64,
    pub eta_minutes: f64,
}

#[derive(Debug, Clone)]
pub enum BatteryState {
    Unknown,
    Charging,
    Discharging,
    NotCharging,
    PendingDischarge,
}

impl From<u32> for BatteryState {
    fn from(value: u32) -> Self {
        match value {
            1 => BatteryState::Charging,
            2 => BatteryState::Discharging,
            3 => BatteryState::NotCharging,
            4 => BatteryState::PendingDischarge,
            _ => BatteryState::Unknown,
        }
    }
}

pub async fn start_battery_listener(tx: Sender<BatteryStats>) -> zbus::Result<()> {
    let connection = Connection::system().await?;
    
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.UPower",
        "/org/freedesktop/UPower/devices/DisplayDevice",
        "org.freedesktop.UPower.Device",
    ).await?;

    // Funzione helper sincrona
    let get_stats = async |p: &Proxy| -> zbus::Result<BatteryStats> {
        let state_val: u32 = p.get_property("State").await?;
        let percentage: f64 = p.get_property("Percentage").await?;
        let time_to_empty: i64 = p.get_property("TimeToEmpty").await?;
        
        let obj = BatteryStats {
            state: BatteryState::from(state_val),
            percentage,
            eta_minutes: (time_to_empty as f64) / 60.0,
        };
        dbg_println!("{obj:?}");
        Ok(obj)
    };

    // Invio iniziale
    if let Ok(stats) = get_stats(&proxy).await {
        dbg_println!("{}", "Battery first info load!".yellow());
        let _ = tx.send(stats);
    }

    // Qui ascoltiamo i segnali in modo bloccante
    // receive_signal_iterator è il metodo per le API blocking
    let mut signal_iterator = proxy.receive_signal("PropertiesChanged").await?;
    
    while let Some(_) = signal_iterator.next().await {
        dbg_println!("{}", "Battery signal!".yellow());
        if let Ok(stats) = get_stats(&proxy).await {
            let _ = tx.send(stats);
        }
    }

    Ok(())
}