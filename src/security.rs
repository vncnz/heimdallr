use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use std::sync::mpsc::Sender;
use serde_json::Value;
use std::collections::HashMap;
use colored::Colorize;

use crate::utils::log_to_file;

#[derive(Debug, Clone, PartialEq)]
pub struct MicCameraStatus {
    pub mic_active: Vec<String>,
    pub camera_active: Vec<String>,
}

pub fn start_pw_monitor(tx: Sender<MicCameraStatus>) -> Result<(), Box<dyn std::error::Error>> {
    std::thread::spawn(move || {
        let mut child = Command::new("pw-dump")
            .arg("--monitor")
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start pw-dump. Ensure pipewire-tools is installed.");

        let stdout = child.stdout.take().unwrap();
        let reader = BufReader::new(stdout);
        
        let stream = serde_json::Deserializer::from_reader(reader).into_iter::<Value>();

        // We use HashMaps to track unique Node IDs so we can add/remove accurately
        let mut active_mics: HashMap<u64, String> = HashMap::new();
        let mut active_cameras: HashMap<u64, String> = HashMap::new();

        for response in stream {
            if let Ok(Value::Array(nodes)) = response {
                let mut changed = false;

                for node in nodes {
                    let id = node["id"].as_u64().unwrap_or(0);
                    let info = &node["info"];
                    let props = &info["props"];
                    let state = info["state"].as_str().unwrap_or("");
                    let media_class = props["media.class"].as_str().unwrap_or("");
                    let app_name = props["application.name"].as_str()
                        .unwrap_or("Unknown App")
                        .to_string();

                    let is_mic = media_class.contains("Stream/Input/Audio");
                    let is_cam = media_class.contains("Stream/Input/Video");

                    if is_mic || is_cam {
                        if state == "running" {
                            if is_mic && !active_mics.contains_key(&id) { active_mics.insert(id, app_name); changed = true; }
                            else if is_cam && !active_cameras.contains_key(&id) { active_cameras.insert(id, app_name); changed = true; }
                        } else {
                            // If state is suspended or idle, remove from active list
                            if active_mics.remove(&id).is_some() { changed = true; }
                            if active_cameras.remove(&id).is_some() { changed = true; }
                        }
                    }
                }

                if changed {
                    // Create the status snapshot from our HashMaps
                    let current_status = MicCameraStatus {
                        mic_active: active_mics.values().cloned().collect(),
                        camera_active: active_cameras.values().cloned().collect(),
                    };
                    log_to_file(format!("{:?}", current_status));
                    
                    // We send a CLONE so the loop can keep its own maps for the next update
                    let _ = tx.send(current_status);
                }
            }
        }
    });
    Ok(())
}