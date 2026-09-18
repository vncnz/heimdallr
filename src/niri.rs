use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use std::sync::{mpsc::Sender, Mutex, OnceLock};
use std::thread;
use std::collections::HashMap;
use serde_json::Value;

use crate::utils::log_to_file;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: u32,
    workspace: i32,
    pos: i32,
    urgent: bool,
    title: String,
    pub appid: String
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInfo {
    pub id: i32,
    pub idx: i32,
    pub name: Option<String>,
    pub output: String,
    pub is_urgent: bool,
    pub is_active: bool,
    pub is_focused: bool,
    pub active_window_id: Option<u32>,
    pub window_count: usize,
}

static NIRI_URGENT_WINDOWS: OnceLock<Mutex<Vec<WindowInfo>>> = OnceLock::new();
static NIRI_WORKSPACES: OnceLock<Mutex<Vec<WorkspaceInfo>>> = OnceLock::new();

fn urgent_windows_store() -> &'static Mutex<Vec<WindowInfo>> {
    NIRI_URGENT_WINDOWS.get_or_init(|| Mutex::new(Vec::new()))
}

fn workspaces_store() -> &'static Mutex<Vec<WorkspaceInfo>> {
    NIRI_WORKSPACES.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn set_urgent_windows(windows: Vec<WindowInfo>) {
    let mut store = urgent_windows_store().lock().unwrap();
    *store = windows;
}

pub fn get_urgent_windows() -> Vec<WindowInfo> {
    urgent_windows_store().lock().unwrap().clone()
}

pub fn set_workspaces(workspaces: Vec<WorkspaceInfo>) {
    let mut store = workspaces_store().lock().unwrap();
    *store = workspaces;
}

pub fn get_workspaces() -> Vec<WorkspaceInfo> {
    workspaces_store().lock().unwrap().clone()
}

fn workspace_count(last_pos: &HashMap<u32, WindowInfo>, workspace_id: i32) -> usize {
    last_pos.values().filter(|info| info.workspace == workspace_id).count()
}

fn parse_workspace_value(value: &Value, last_pos: &HashMap<u32, WindowInfo>) -> WorkspaceInfo {
    let id = value.get("id").and_then(Value::as_i64).map(|n| n as i32).unwrap_or(0);
    let idx = value.get("idx").and_then(Value::as_i64).map(|n| n as i32).unwrap_or(0);
    let name = value.get("name").and_then(Value::as_str).map(str::to_owned);
    let output = value.get("output").and_then(Value::as_str).unwrap_or("").to_string();
    let is_urgent = value.get("is_urgent").and_then(Value::as_bool).unwrap_or(false);
    let is_active = value.get("is_active").and_then(Value::as_bool).unwrap_or(false);
    let is_focused = value.get("is_focused").and_then(Value::as_bool).unwrap_or(false);
    let active_window_id = value.get("active_window_id").and_then(Value::as_u64).map(|n| n as u32);

    WorkspaceInfo {
        id,
        idx,
        name,
        output,
        is_urgent,
        is_active,
        is_focused,
        active_window_id,
        window_count: workspace_count(last_pos, id),
    }
}

fn emit_workspace_snapshot(
    tx_workspaces: &Sender<Vec<WorkspaceInfo>>,
    workspace_state: &HashMap<i32, WorkspaceInfo>,
    last_pos: &HashMap<u32, WindowInfo>,
) {
    let mut workspaces: Vec<WorkspaceInfo> = workspace_state.values().cloned().collect();
    for workspace in &mut workspaces {
        workspace.window_count = workspace_count(last_pos, workspace.id);
    }
    workspaces.sort_by(|a, b| a.idx.cmp(&b.idx).then(a.id.cmp(&b.id)));
    set_workspaces(workspaces.clone());
    let _ = tx_workspaces.send(workspaces);
}

pub fn focus_next_urgent_window() -> Result<(), String> {
    let mut urgent = get_urgent_windows();
    if urgent.is_empty() {
        return Err("No urgent windows".to_string());
    }

    urgent.sort_by(|a, b| {
        a.workspace
            .cmp(&b.workspace)
            .then(a.pos.cmp(&b.pos))
            .then(a.id.cmp(&b.id))
    });

    let target = urgent.first().ok_or_else(|| "No urgent windows".to_string())?;
    let status = Command::new("niri")
        .args(["msg", "action", "focus-window", "--id", &target.id.to_string()])
        .status()
        .map_err(|err| format!("Failed to execute niri focus command: {err}"))?;

    if !status.success() {
        return Err(format!("niri focus-window {} exited with status {:?}", target.id, status.code()));
    }

    Ok(())
}

pub fn handle_niri_command(cmd: &str) -> Result<(), String> {
    match cmd {
        "focus_next_urgent_window" => focus_next_urgent_window(),
        _ => Err(format!("Unknown Niri command: {cmd}")),
    }
}

/// Start a listener for niri events.
/// Sends urgent windows on `tx` and the complete workspace snapshot on `tx_workspaces`.
pub fn start_niri_listener(
    tx: Sender<Vec<WindowInfo>>,
    tx_workspaces: Sender<Vec<WorkspaceInfo>>,
) -> Result<(), Box<dyn std::error::Error>> {
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
        let mut workspace_state: HashMap<i32, WorkspaceInfo> = HashMap::new();

        for line_res in reader.lines() {
            if let Ok(line) = line_res {
                if line.trim().is_empty() { continue; }

                // Try JSON parsing first (niri --json emits objects like {"WindowOpenedOrChanged":{...}})
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    if let Some(ev) = v.get("WorkspacesChanged") {
                        if let Some(workspaces) = ev.get("workspaces").and_then(Value::as_array) {
                            workspace_state.clear();
                            for workspace_value in workspaces {
                                let workspace = parse_workspace_value(workspace_value, &last_pos);
                                workspace_state.insert(workspace.id, workspace);
                            }
                            emit_workspace_snapshot(&tx_workspaces, &workspace_state, &last_pos);
                            log_to_file(format!("niri: workspaces changed -> {:?}", workspace_state.values().collect::<Vec<_>>()));
                            continue;
                        }
                    }

                    if let Some(ev) = v.get("WorkspaceActivated") {
                        let id = ev.get("id").and_then(Value::as_i64).map(|n| n as i32);
                        let focused = ev.get("focused").and_then(Value::as_bool).unwrap_or(false);
                        if let Some(id) = id {
                            match workspace_state.get_mut(&id) {
                                Some(ws) => {
                                    ws.is_focused = focused;
                                    ws.is_active = true;
                                },
                                None => {
                                    let workspace = WorkspaceInfo {
                                        id,
                                        idx: 0,
                                        name: None,
                                        output: "".into(),
                                        is_urgent: false,
                                        is_active: true,
                                        is_focused: focused,
                                        active_window_id: None,
                                        window_count: workspace_count(&last_pos, id),
                                    };
                                    workspace_state.insert(id, workspace);
                                }
                            }
                            emit_workspace_snapshot(&tx_workspaces, &workspace_state, &last_pos);
                            log_to_file(format!("niri: workspace {} activated focused={}", id, focused));
                            continue;
                        }
                    }

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
                                let appid = win.get("app_id").and_then(|t| t.as_str()).unwrap_or("").to_string();
                                last_pos.insert(id, WindowInfo { id, workspace, pos: pos0, urgent: is_urgent, title, appid });
                                emit_workspace_snapshot(&tx_workspaces, &workspace_state, &last_pos);
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
                            emit_workspace_snapshot(&tx_workspaces, &workspace_state, &last_pos);
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
                                last_pos.insert(id, WindowInfo { id, workspace: 0, pos: 0, urgent, title: "".into(), appid: "".into() });
                                log_to_file(format!("niri: inserted urgency for unknown window {} -> {}", id, urgent));
                            }
                            let urgent_windows: Vec<WindowInfo> = last_pos.values().filter(|el| el.urgent).cloned().collect();
                            set_urgent_windows(urgent_windows.clone());
                            let _ = tx.send(urgent_windows);
                            emit_workspace_snapshot(&tx_workspaces, &workspace_state, &last_pos);
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