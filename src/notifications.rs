use serde_json::Value;
use zbus::{ConnectionBuilder, dbus_interface, zvariant};
use std::{collections::HashMap, sync::{Arc, Mutex, mpsc::Sender}, time::{Duration, Instant}};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::utils::log_to_file;

static NEXT_ID: AtomicU32 = AtomicU32::new(2);

fn generate_id() -> u32 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub urgency: u8,
    pub received_at: Instant,
    pub expired_at: Option<Instant>,
    pub app_icon: String,
    pub id: u32,
    pub unmounting: bool,
    pub reboot: bool,
    pub replaces_id: u32
}

#[derive(Clone)]
struct NotificationServer {
    notifications: Arc<Mutex<Vec<Notification>>>,
    tx: Sender<Notification>
}

fn get_u8(map: &HashMap<String, zvariant::Value<'_>>, key: &str) -> u8 {
    map.get(key)
        .and_then(|v| {
            v.downcast_ref::<u8>().copied()
                .or_else(|| v.downcast_ref::<u16>().and_then(|&n| u8::try_from(n).ok()))
                .or_else(|| v.downcast_ref::<u32>().and_then(|&n| u8::try_from(n).ok()))
        })
        .unwrap_or(0)
}


#[dbus_interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        _actions: Vec<String>,
        hints: std::collections::HashMap<String, zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> u32 {
        let msg = format!("app_name:{app_name} summary:{summary} body:{body} timeout:{expire_timeout} hints:{hints:?} replaces_id:{replaces_id}");
        log_to_file(msg);

        // let u = hints.get("urgency").and_then(Value::as_u64).and_then(|n|u8::try_from(n).ok()).unwrap_or(0);
        // let urgency = hints.get("urgency").unwrap().clone().downcast().expect("No urgency");
        let urgency: u8 = get_u8(&hints, "urgency");
        let expired_at = if expire_timeout > 1 { Some(Instant::now() + Duration::new(expire_timeout as u64, 0)) } 
                                else if urgency < 2 { Some(Instant::now() + Duration::new(3, 0)) }
                                else { None };
        // *list = list.iter().filter(|notif| notif.expired_at > Instant::now()).map(|item|item.to_owned()).collect();

        let id = generate_id();
        let new_notif = Notification {
            app_name: app_name.into(),
            summary: summary.into(),
            body: body.into(),
            urgency,
            received_at: Instant::now(),
            expired_at: expired_at,
            app_icon: app_icon.into(),
            id,
            replaces_id,
            unmounting: summary.contains("Unmounting"),
            reboot: summary.contains("Reboot recommended")
        };
        let _ = self.tx.send(new_notif);

        

        /* use std::thread;
        thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(3000 + 10));
            self.tx.send(list.iter().filter(|notif| notif.expired_at > Instant::now()).map(|item|item.to_owned()).collect());
        }); */

        // ID arbitrario della notifica (di solito crescente)
        println!("Returning {}", id);
        id
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "Heimdallr".to_string(),
            "Vincenzo".to_string(),
            "1.0".to_string(),
            "1.2".to_string(),
        )
    }
}

pub async fn start_notification_listener(tx: Sender<Notification>) -> zbus::Result<()> {
    let server = NotificationServer {
        notifications: Arc::new(Mutex::new(vec![])),
        tx
    };

    let _conn = ConnectionBuilder::session()?
        .name("org.freedesktop.Notifications")?
        .serve_at("/org/freedesktop/Notifications", server)?
        .build()
        .await?;

    println!("Heimdallr is now listening to notifications!");
    loop {
        std::thread::park();
    }
}
