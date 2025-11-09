use zbus::{ConnectionBuilder, dbus_interface, zvariant};
use std::{ops::Not, sync::{Arc, Mutex, mpsc::Sender}, time::{Duration, Instant}};

#[derive(Debug, Clone)]
pub struct Notification {
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub urgency: u8,
    pub arrived_at: Instant,
    pub expired_at: Option<Instant>,
    pub app_icon: String
}

#[derive(Clone)]
struct NotificationServer {
    notifications: Arc<Mutex<Vec<Notification>>>,
    tx: Sender<Vec<Notification>>
}

#[dbus_interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    fn notify(
        &self,
        app_name: &str,
        _replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        _actions: Vec<String>,
        hints: std::collections::HashMap<String, zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> u32 {
        let mut list = self.notifications.lock().unwrap();
        let msg = format!("app_name:{app_name} summary:{summary} body:{body} timeout:{expire_timeout} hints:{hints:?}");
        println!("🔔 Nuova notifica: {msg}");

        let urgency = hints.get("urgency").unwrap().clone().downcast().expect("No urgency");
        let expired_at = if expire_timeout > 1 { Some(Instant::now() + Duration::new(expire_timeout as u64, 0)) } 
                                else if urgency < 2 { Some(Instant::now() + Duration::new(3, 0)) }
                                else { None };
        // *list = list.iter().filter(|notif| notif.expired_at > Instant::now()).map(|item|item.to_owned()).collect();

        list.push(Notification {
            app_name: app_name.into(),
            summary: summary.into(),
            body: body.into(),
            urgency,
            arrived_at: Instant::now(),
            expired_at: expired_at,
            app_icon: app_icon.into()
        });
        let _ = self.tx.send(list.clone());

        let list_clone = Arc::clone(&self.notifications);
        let tx_clone = self.tx.clone();

        if expired_at.is_some() {
            use std::thread;
            thread::spawn(move || {
                let remaining = expired_at.unwrap().saturating_duration_since(Instant::now());
                thread::sleep(remaining + Duration::from_millis(10)); // un piccolo margine
                let mut vec = list_clone.lock().unwrap();
                vec.retain(|n| n.expired_at.is_none() || (n.expired_at.unwrap() > Instant::now()));
                let _ = tx_clone.send(vec.clone());
            });
        }

        /* use std::thread;
        thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(3000 + 10));
            self.tx.send(list.iter().filter(|notif| notif.expired_at > Instant::now()).map(|item|item.to_owned()).collect());
        }); */

        // ID arbitrario della notifica (di solito crescente)
        list.len() as u32
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

pub async fn start_notification_listener(tx: Sender<Vec<Notification>>) -> zbus::Result<()> {
    let server = NotificationServer {
        notifications: Arc::new(Mutex::new(vec![])),
        tx
    };

    let _conn = ConnectionBuilder::session()?
        .name("org.freedesktop.Notifications")?
        .serve_at("/org/freedesktop/Notifications", server)?
        .build()
        .await?;

    println!("Heimdallr ora serve come notificatore DBus!");
    loop {
        std::thread::park();
    }
}
