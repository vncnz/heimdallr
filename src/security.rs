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

                    if info.is_null() {
                        if active_mics.remove(&id).is_some() { changed = true; }
                        if active_cameras.remove(&id).is_some() { changed = true; }
                        continue;
                    }

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
                            if is_mic { active_mics.insert(id, app_name); changed = true; }
                            else if is_cam { active_cameras.insert(id, app_name); changed = true; }
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




// Camera in use detection by checking /proc for processes with open file descriptors to /dev/video* devices.
// This is a more direct method that doesn't rely on PipeWire, but it may not catch all cases (e.g., if a process has the camera open but isn't actively using it).
// Pipewire can manage cameras, but several systems and applications might access the camera directly, so this method can serve as a complementary check to ensure we catch all active camera usage.

use std::fs;
use std::path::Path;

pub fn is_camera_in_use() -> bool {
    // Webcams are usually /dev/video0, video2, etc.
    let camera_devices = ["/dev/video0", "/dev/video1", "/dev/video2", "/dev/video4"];
    
    // 1. Get a list of all process IDs from /proc
    let proc_dir = match fs::read_dir("/proc") {
        Ok(dir) => dir,
        Err(_) => return false,
    };

    for entry in proc_dir.flatten() {
        let name = entry.file_name();
        let s_name = name.to_string_lossy();
        
        // Skip non-process directories
        if !s_name.chars().all(|c| c.is_numeric()) {
            continue;
        }

        // 2. Check the File Descriptors (fd) for this process
        let fd_path = format!("/proc/{}/fd", s_name);
        if let Ok(fds) = fs::read_dir(fd_path) {
            for fd in fds.flatten() {
                // 3. See where the symlink points
                if let Ok(target) = fs::read_link(fd.path()) {
                    let target_str = target.to_string_lossy();
                    for dev in &camera_devices {
                        if target_str == *dev {
                          eprintln!("Camera {} in use by PID {}: {}", dev, s_name, target_str);
                            return true; // Found a match!
                        }
                    }
                }
            }
        }
    }
    false
}
