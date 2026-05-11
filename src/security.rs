use std::process::{Command, Stdio};
use std::io::BufReader;
use std::sync::mpsc::Sender;
use serde_json::Value;
use std::collections::HashMap;
// use colored::Colorize;

use crate::utils::log_to_file;

#[derive(Debug, Clone, PartialEq)]
pub struct MicCameraStatus {
    pub mic_active: Vec<String>,
    pub camera_active: Vec<String>,
    pub pristine: bool
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
                        pristine: true
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

/* pub fn is_camera_in_use() -> bool {
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
} */

















// NEW SYSTEM

use std::collections::{/*HashMap, */HashSet};
// use std::fs;
// use std::process::Command;
use std::sync::mpsc::{self/*, Sender*/};
use std::time::Duration;
use glob::glob;

pub fn start_security_monitor(tx: Sender<MicCameraStatus>) -> Result<(), Box<dyn std::error::Error>> {
    // Internal channel to get data from the PipeWire event thread
    let (pw_tx, pw_rx) = mpsc::channel::<MicCameraStatus>();
    
    // 1. Launch the PipeWire Event Listener (Reactive)
    // This is the logic we refined earlier using pw-dump --monitor
    let _ = start_pw_monitor(pw_tx);

    // 2. Launch the Hardware Polling Thread (Proactive Security)
    std::thread::spawn(move || {
        let mut last_sent_status: Option<MicCameraStatus> = None;
        let mut pw_context = MicCameraStatus { mic_active: vec![], camera_active: vec![], pristine: false };
        let mut trusted_pids = HashSet::new();

        loop {
            // Update metadata from PipeWire if available (non-blocking)
            while let Ok(update) = pw_rx.try_recv() {
                pw_context = update;
            }

            let mut current_status = MicCameraStatus {
                mic_active: Vec::new(),
                camera_active: Vec::new(),
                pristine: true
            };

            // --- PART A: ABSOLUTE MIC TRUTH (/proc/asound) ---
            // Check if any hardware capture device is physically RUNNING
            let mut hw_mic_active = false;
            if let Ok(paths) = glob("/proc/asound/card*/pcm*c/sub*/status") {
                for path in paths.flatten() {
                    if let Ok(content) = fs::read_to_string(path) {
                        if content.contains("state: RUNNING") {
                            hw_mic_active = true;
                            break;
                        }
                    }
                }
            }

            // --- PART B: PROCESS SCAN (/proc/[pid]/fd) ---
            let mut direct_mic_pids = Vec::new();
            let mut direct_cam_pids = Vec::new();

            if let Ok(entries) = fs::read_dir("/proc") {
                for entry in entries.flatten() {
                    let pid_str = entry.file_name().to_string_lossy().into_owned();
                    if !pid_str.chars().all(|c| c.is_numeric()) { continue; }
                    
                    let pid: u32 = pid_str.parse().unwrap_or(0);
                    let fd_path = format!("/proc/{}/fd", pid_str);

                    if let Ok(fds) = fs::read_dir(fd_path) {
                        for fd in fds.flatten() {
                            if let Ok(target) = fs::read_link(fd.path()) {
                                let target_str = target.to_string_lossy();
                                
                                // Check for Camera hardware
                                if target_str.contains("/dev/video") {
                                    direct_cam_pids.push(pid);
                                }
                                // Check for ALSA Audio hardware
                                if target_str.contains("/dev/snd/pcm") && target_str.ends_with('c') {
                                    direct_mic_pids.push(pid);
                                }
                            }
                        }
                    }
                }
            }

            // --- PART C: ATTRIBUTION & SECURITY LOGIC ---
            
            // Resolve Camera
            for pid in direct_cam_pids {
                let name = get_process_name(pid, &mut trusted_pids);
                current_status.camera_active.push(name);
            }

            // Resolve Mic: If HW is running but no one owns the FD, it's a ghost/rootkit
            if hw_mic_active && direct_mic_pids.is_empty() {
                current_status.mic_active.push("Kernel-level/Hidden Recorder!".to_string());
            } else {
                for pid in direct_mic_pids {
                    let name = get_process_name(pid, &mut trusted_pids);
                    // If it's a known sound server, use PipeWire's rich metadata
                    if name == "pipewire" || name == "wireplumber" {
                        current_status.mic_active.extend(pw_context.mic_active.clone());
                    } else {
                        current_status.mic_active.push(format!("{} (ALSA Bypass)", name));
                    }
                }
            }

            // Clean duplicates (e.g. multiple FDs from same process)
            current_status.mic_active.sort();
            current_status.mic_active.dedup();
            current_status.camera_active.sort();
            current_status.camera_active.dedup();

            // Only notify if state changed
            if Some(&current_status) != last_sent_status.as_ref() {
                if last_sent_status.is_some() || current_status.mic_active.is_empty() == false || current_status.camera_active.is_empty() == false { // avoid first event if useless
                    let _ = tx.send(current_status.clone());
                }
                last_sent_status = Some(current_status);
            }

            std::thread::sleep(Duration::from_secs(2));
        }
    });

    Ok(())
}

fn get_process_name(pid: u32, trusted: &mut HashSet<u32>) -> String {
    if trusted.contains(&pid) {
        // We know these PIDs are system services, return a placeholder to save IO
        // but in a real app, you might want the actual comm name once.
    }
    fs::read_to_string(format!("/proc/{}/comm", pid))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| format!("PID {}", pid))
}