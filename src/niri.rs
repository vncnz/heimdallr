use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use std::sync::mpsc::Sender;
use std::thread;
use regex::Regex;
use std::collections::HashMap;
use serde_json::Value;

use crate::utils::log_to_file;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    workspace: i32,
    pos: i32,
    urgent: bool,
    title: String
}

/// Start a listener for niri events.
/// Sends `Some(window_id)` when a window needs attention, or `None` when there is no such window.
pub fn start_niri_listener(tx: Sender<Vec<WindowInfo>>) -> Result<(), Box<dyn std::error::Error>> {
    thread::spawn(move || {
        // Try to spawn the external `niri-ipc subscribe` command. If it's not available,
        // log and return so the rest of the app keeps working.
        let mut child = match Command::new("niri").arg("msg").arg("--json").arg("event-stream").stdout(Stdio::piped()).spawn() {
            Ok(c) => c,
            Err(e) => {
                log_to_file(format!("Failed to start niri event stream: {:?}.", e));
                return;
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                log_to_file("niri had no stdout".to_string());
                return;
            }
        };

        let reader = BufReader::new(stdout);

        // Keep last workspace, position and urgency for each open window
        let mut last_pos: HashMap<u32, WindowInfo> = HashMap::new();
        // let mut focused_window: u32 = 0;

        // Regex to extract the first integer window id we find in an event line (fallback)
        // let id_re = Regex::new(r"(\d+)").unwrap();

        for line_res in reader.lines() {
            if let Ok(line) = line_res {
                if line.trim().is_empty() { continue; }

                // Try JSON parsing first (niri --json emits objects like {"WindowOpenedOrChanged":{...}})
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    
                    // WindowOpenedOrChanged: update hashmap with workspace_id and pos_in_scrolling_layout[0]
                    if let Some(ev) = v.get("WindowOpenedOrChanged") {
                        if let Some(win) = ev.get("window") {
                            if let Some(id_v) = win.get("id").and_then(|x| x.as_u64()) {
                                let id = id_v as u32;
                                let workspace = win.get("workspace_id").and_then(|x| x.as_i64()).map(|n| n as i32).unwrap_or(0);
                                let mut pos0: i32 = 0;
                                if let Some(layout) = win.get("layout") {
                                    if let Some(pos_arr) = layout.get("pos_in_scrolling_layout").and_then(|p| p.as_array()) {
                                        if let Some(first) = pos_arr.get(0) {
                                            if let Some(n) = first.as_i64() { pos0 = n as i32; }
                                        }
                                    }
                                }
                                let is_urgent = win.get("is_urgent").and_then(|b| b.as_bool()).unwrap_or(false);
                                let title = win.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();
                                last_pos.insert(id, WindowInfo { workspace, pos: pos0, urgent: is_urgent, title });
                                log_to_file(format!("niri: window {} opened/changed ws={} pos={} urgent={}", id, workspace, pos0, is_urgent));
                                continue;
                            }
                        }
                    }

                    // WindowClosed: remove from hashmap
                    if let Some(ev) = v.get("WindowClosed") {
                        if let Some(id_v) = ev.get("id").and_then(|x| x.as_u64()) {
                            let id = id_v as u32;
                            last_pos.remove(&id);
                            log_to_file(format!("niri: window {} closed, removed from map", id));
                            continue;
                        }
                    }

                    if let Some(ev) = v.get("WindowUrgencyChanged") {
                        // Update or insert urgency state for the given window id
                        if let Some(id_v) = ev.get("id").and_then(|x| x.as_u64()) {
                            let id = id_v as u32;
                            let urgent = ev.get("urgent").and_then(|x| x.as_bool()).unwrap_or(true);
                            if let Some(info) = last_pos.get_mut(&id) {
                                info.urgent = urgent;
                                log_to_file(format!("niri: updated urgency for window {} -> {}", id, urgent));
                            } else {
                                last_pos.insert(id, WindowInfo { workspace: 0, pos: 0, urgent, title: "".into() });
                                log_to_file(format!("niri: inserted urgency for unknown window {} -> {}", id, urgent));
                            }
                            let _ = tx.send(last_pos.values().filter(|el|el.urgent).cloned().collect());
                            continue;
                        }
                    }

                    /* if let Some(ev) = v.get("WindowFocusChanged") {
                        if let Some(id_v) = ev.get("id").and_then(|x| x.as_u64()) {
                            let id = id_v as u32;
                            focused_window = id;
                            continue;
                        }
                    } */
                }

                // Fallback: handle old textual events (urgency/focus)
                if line.contains("FocusChanged") || line.contains("WindowFocusChanged") {
                    // let _ = tx.send(vec![]);
                } else if line.contains("WindowUrgencyCleared") {
                    // let _ = tx.send(vec![]);
                } else {
                    log_to_file(format!("niri event ignored: {}", line));
                }
            } else {
                break;
            }
        }
    });

    Ok(())
}


/* use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use std::sync::mpsc::Sender;
use std::thread;
use std::collections::HashMap;
use serde_json::Value;

use crate::utils::log_to_file;

/// Start a listener for niri events.
/// Sends `Some(window_id)` when a window needs attention, or `None` when there is no such window.
pub fn start_niri_listener(tx: Sender<Option<u32>>) -> Result<(), Box<dyn std::error::Error>> {
    thread::spawn(move || {
        // Try to spawn the external `niri-ipc subscribe` command. If it's not available,
        // log and return so the rest of the app keeps working.
        let mut child = match Command::new("niri").arg("msg").arg("--json").arg("event-stream").stdout(Stdio::piped()).spawn() {
            Ok(c) => c,
            Err(e) => {
                log_to_file(format!("Failed to start niri event stream: {:?}.", e));
                return;
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                log_to_file("niri had no stdout".to_string());
                return;
            }
        };

        let reader = BufReader::new(stdout);

        // Keep last workspace, position and urgency for each open window
        #[derive(Debug, Clone)]
        struct WindowInfo {
            workspace: i32,
            pos: i32,
            urgent: bool,
        }

        let mut last_pos: HashMap<u32, WindowInfo> = HashMap::new();
        let mut focused_window: u32 = 0;

        // Regex to extract the first integer window id we find in an event line (fallback)
        // let id_re = Regex::new(r"(\d+)").unwrap();

        for line_res in reader.lines() {
            if let Ok(line) = line_res {
                if line.trim().is_empty() { continue; }

                // Try JSON parsing first (niri --json emits objects like {"WindowOpenedOrChanged":{...}})
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    
                    // WindowOpenedOrChanged: update hashmap with workspace_id and pos_in_scrolling_layout[0]
                    if let Some(ev) = v.get("WindowOpenedOrChanged") {
                        if let Some(win) = ev.get("window") {
                            if let Some(id_v) = win.get("id").and_then(|x| x.as_u64()) {
                                let id = id_v as u32;
                                let workspace = win.get("workspace_id").and_then(|x| x.as_i64()).map(|n| n as i32).unwrap_or(0);
                                let mut pos0: i32 = 0;
                                if let Some(layout) = win.get("layout") {
                                    if let Some(pos_arr) = layout.get("pos_in_scrolling_layout").and_then(|p| p.as_array()) {
                                        if let Some(first) = pos_arr.get(0) {
                                            if let Some(n) = first.as_i64() { pos0 = n as i32; }
                                        }
                                    }
                                }
                                let is_urgent = win.get("is_urgent").and_then(|b| b.as_bool()).unwrap_or(false);
                                last_pos.insert(id, WindowInfo { workspace, pos: pos0, urgent: is_urgent });
                                log_to_file(format!("niri: window {} opened/changed ws={} pos={} urgent={}", id, workspace, pos0, is_urgent));
                                continue;
                            }
                        }
                    }

                    // WindowClosed: remove from hashmap
                    if let Some(ev) = v.get("WindowClosed") {
                        if let Some(id_v) = ev.get("id").and_then(|x| x.as_u64()) {
                            let id = id_v as u32;
                            last_pos.remove(&id);
                            log_to_file(format!("niri: window {} closed, removed from map", id));
                            continue;
                        }
                    }

                    if let Some(ev) = v.get("WindowUrgencyChanged") {
                        // Update or insert urgency state for the given window id
                        if let Some(id_v) = ev.get("id").and_then(|x| x.as_u64()) {
                            let id = id_v as u32;
                            let urgent = ev.get("urgent").and_then(|x| x.as_bool()).unwrap_or(true);
                            if let Some(info) = last_pos.get_mut(&id) {
                                info.urgent = urgent;
                                log_to_file(format!("niri: updated urgency for window {} -> {}", id, urgent));
                            } else {
                                last_pos.insert(id, WindowInfo { workspace: 0, pos: 0, urgent });
                                log_to_file(format!("niri: inserted urgency for unknown window {} -> {}", id, urgent));
                            }
                            let _ = tx.send(Some(id));
                            continue;
                        }
                    }

                    if let Some(ev) = v.get("WindowFocusChanged") {
                        if let Some(id_v) = ev.get("id").and_then(|x| x.as_u64()) {
                            let id = id_v as u32;
                            focused_window = id;
                            continue;
                        }
                    }
                }

                // Fallback: handle old textual events (urgency/focus)
                if line.contains("FocusChanged") || line.contains("WindowFocusChanged") {
                    let _ = tx.send(None);
                } else if line.contains("WindowUrgencyCleared") {
                    let _ = tx.send(None);
                } else {
                    log_to_file(format!("niri event ignored: {}", line));
                }
            } else {
                break;
            }
        }
    });

    Ok(())
}
*/