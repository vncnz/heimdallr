use zbus::{ConnectionBuilder, dbus_interface, zvariant::{self, ObjectPath, Value}};
use std::{collections::HashMap, sync::mpsc::Sender, time::{Duration, Instant}};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::utils::log_to_file;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BatteryState {
    Unknown,
    Charging,
    Discharging,
    NotCharging, // Mappato da Full/PendingCharge
    PendingDischarge,
}

#[derive(Debug, Clone)]
pub struct BatteryStats {
    pub state: BatteryState,
    pub percentage: f64,
    pub eta_minutes: f64, // Tempo rimanente in minuti (0.0 se non applicabile)
}


#[derive(Clone)]
struct BatteryServer {
    tx: Sender<BatteryStats>
}

#[dbus_interface(name = "org.freedesktop.UPower.Device")]
impl BatteryServer {
    fn changed (
        &self,
        object: ObjectPath<'_>,
        properties: HashMap<String, Value<'_>>) {

        eprintln!("Event from dbus");
    }
}

pub async fn start_battery_listener(tx: Sender<BatteryStats>) -> zbus::Result<()> {
    let server = BatteryServer {
        // notifications: Arc::new(Mutex::new(vec![])),
        tx
    };

    let _conn = ConnectionBuilder::session()?
        .name("org.freedesktop.UPower.Device")?
        .serve_at("/org/freedesktop/UPower.Device", server)?
        .build()
        .await?;

    println!("Heimdallr is now listening to upower events!");
    loop {
        std::thread::park();
    }
}
