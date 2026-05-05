use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use std::sync::mpsc::Sender;
use serde_json::Value;
use colored::Colorize;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MicCameraStatus {
    pub mic_active: bool,
    pub camera_active: bool,
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
        
        // Use a Stream Deserializer to handle a continuous stream of JSON arrays
        let stream = serde_json::Deserializer::from_reader(reader).into_iter::<Value>();

        for response in stream {
            if let Ok(Value::Array(nodes)) = response {
                let mut status = MicCameraStatus { mic_active: false, camera_active: false };

                for node in nodes {
                    let info = &node["info"];
                    let props = &info["props"];
                    let params = &info["params"];
                    let tag = &params["Tag"];
                    // let direction = &tag["direction"];

                    // eprintln!("{}", format!("Node params: {:?}", params).red());
                    // eprintln!("{}", format!("Node direction: {:?}", direction).red());

                    //for item in tag.as_array().unwrap_or(&vec![]) {
                        //if item["direction"] == "Input" {

                            if let Some(media_class) = props["media.class"].as_str() {
                                let media_type = if media_class.contains("Stream/Input/Audio") {
                                    "Audio"
                                } else if media_class.contains("Stream/Input/Video") {
                                    "Video"
                                } else {
                                    "Other"
                                };
                                
                                if media_type != "Other" {
                                    
                                    if info["state"] == "running" {
                                        eprintln!("{}", format!("Active input node: {:?} ({:?}) for {}", props["application.name"], props["media.class"], info["state"]).red());
                                    } else {
                                        eprintln!("{}", format!("Inactive input node: {:?} ({:?}) for {}", props["application.name"], props["media.class"], info["state"]).yellow());
                                    }
                                    eprintln!("{}", format!("Tag item input: {:?}", node).green());
                                }
                            }
                        //}
                    //}
                    
                    // Check if the node is currently "running" (actively streaming)
                    let is_running = info["state"] == "running";
                    
                    if is_running {
                        if let Some(media_class) = props["media.class"].as_str() {
                            if media_class.contains("Stream/Input/Audio") {
                                status.mic_active = true;
                            }
                            if media_class.contains("Stream/Input/Video") {
                                status.camera_active = true;
                            }
                        }
                    }
                }
                // Only send the update via the channel
                let _ = tx.send(status);
            }
        }
    });
    Ok(())
}



/*
{
    "id": 67,
    "type": "PipeWire:Interface:Node",
    "version": 3,
    "permissions": [ "r", "w", "x", "m" ],
    "info": {
      "max-input-ports": 0,
      "max-output-ports": 129,
      "change-mask": [ "state" ],
      "n-input-ports": 0,
      "n-output-ports": 2,
      "state": "idle",
      "error": null,
      "props": {
        "adapt.follower.spa-node": "",
        "application.language": "en_US.UTF-8",
        "application.name": "Firefox",
        "application.process.binary": "firefox",
        "application.process.host": "Darlene",
        "application.process.id": 2487699,
        "application.process.machine-id": "23f577ff0a7a4a8aa5dbaa72ef0fbdef",
        "application.process.session-id": 2,
        "application.process.user": "vncnz",
        "client.api": "pipewire-pulse",
        "client.id": 68,
        "clock.quantum-limit": 8192,
        "factory.id": 7,
        "library.name": "audioconvert/libspa-audioconvert",
        "media.class": "Stream/Output/Audio",
        "media.name": "Checking your Browser…",
        "node.autoconnect": true,
        "node.latency": "900/48000",
        "node.loop.name": "data-loop.0",
        "node.name": "Firefox",
        "node.rate": "1/48000",
        "node.want-driver": true,
        "object.id": 67,
        "object.register": false,
        "object.serial": 92962,
        "port.group": "stream.0",
        "pulse.attr.maxlength": 4194304,
        "pulse.attr.minreq": 2400,
        "pulse.attr.prebuf": 9608,
        "pulse.attr.tlength": 12000,
        "pulse.corked": true,
        "pulse.server.type": "unix",
        "stream.is-live": true,
        "window.x11.display": ":1"
      },
      "params": {
        "EnumFormat": [
          {
            "mediaType": "audio",
            "mediaSubtype": "raw",
            "format": "F32LE",
            "rate": 48000,
            "channels": 2,
            "position": [ "FL", "FR" ]
          }
        ],
        "PropInfo": [
          {
            "id": "volume",
            "description": "Volume",
            "type": { "default": 1.000000, "min": 0.000000, "max": 10.000000 }
          },
          {
            "id": "mute",
            "description": "Mute",
            "type": {
              "default": false,
              "alt1": false,
              "alt2": true
            }
          },
          {
            "id": "channelVolumes",
            "description": "Channel Volumes",
            "type": { "default": 1.000000, "min": 0.000000, "max": 10.000000 },
            "container": "Array"
          },
          {
            "id": "channelMap",
            "description": "Channel Map",
            "type": "",
            "container": "Array"
          },
          {
            "id": "monitorMute",
            "description": "Monitor Mute",
            "type": {
              "default": false,
              "alt1": false,
              "alt2": true
            }
          },
          {
            "id": "monitorVolumes",
            "description": "Monitor Volumes",
            "type": { "default": 1.000000, "min": 0.000000, "max": 10.000000 },
            "container": "Array"
          },
          {
            "id": "softMute",
            "description": "Soft Mute",
            "type": {
              "default": false,
              "alt1": false,
              "alt2": true
            }
          },
          {
            "id": "softVolumes",
            "description": "Soft Volumes",
            "type": { "default": 1.000000, "min": 0.000000, "max": 10.000000 },
            "container": "Array"
          },
          {
            "name": "monitor.channel-volumes",
            "description": "Monitor channel volume",
            "type": {
              "default": false,
              "alt1": false,
              "alt2": true
            },
            "params": true
          },
          {
            "name": "channelmix.disable",
            "description": "Disable Channel mixing",
            "type": {
              "default": false,
              "alt1": false,
              "alt2": true
            },
            "params": true
          },
          {
            "name": "channelmix.min-volume",
            "description": "Minimum volume level",
            "type": { "default": 0.000000, "min": 0.000000, "max": 10.000000 },
            "params": true
          },
          {
            "name": "channelmix.max-volume",
            "description": "Maximum volume level",
            "type": { "default": 10.000000, "min": 0.000000, "max": 10.000000 },
            "params": true
          },
          {
            "name": "channelmix.normalize",
            "description": "Normalize Volumes",
            "type": {
              "default": false,
              "alt1": false,
              "alt2": true
            },
            "params": true
          },
          {
            "name": "channelmix.mix-lfe",
            "description": "Mix LFE into channels",
            "type": {
              "default": true,
              "alt1": true,
              "alt2": false
            },
            "params": true
          },
          {
            "name": "channelmix.upmix",
            "description": "Enable upmixing",
            "type": {
              "default": true,
              "alt1": true,
              "alt2": false
            },
            "params": true
          },
          {
            "name": "channelmix.lfe-cutoff",
            "description": "LFE cutoff frequency",
            "type": { "default": 0.000000, "min": 0.000000, "max": 1000.000000 },
            "params": true
          },
          {
            "name": "channelmix.fc-cutoff",
            "description": "FC cutoff frequency (Hz)",
            "type": { "default": 0.000000, "min": 0.000000, "max": 48000.000000 },
            "params": true
          },
          {
            "name": "channelmix.rear-delay",
            "description": "Rear channels delay (ms)",
            "type": { "default": 0.000000, "min": 0.000000, "max": 1000.000000 },
            "params": true
          },
          {
            "name": "channelmix.stereo-widen",
            "description": "Stereo widen",
            "type": { "default": 0.000000, "min": 0.000000, "max": 1.000000 },
            "params": true
          },
          {
            "name": "channelmix.hilbert-taps",
            "description": "Taps for phase shift of rear",
            "type": { "default": 0, "min": 0, "max": 255 },
            "params": true
          },
          {
            "name": "channelmix.upmix-method",
            "description": "Upmix method to use",
            "type": "none",
            "params": true,
            "labels": [
              "none",
              "Disabled",
              "simple",
              "Simple upmixing",
              "psd",
              "Passive Surround Decoding"
            ]
          },
          {
            "id": "rate",
            "description": "Rate scaler",
            "type": { "default": 1.000000, "min": 0.000000, "max": 10.000000 }
          },
          {
            "id": "quality",
            "name": "resample.quality",
            "description": "Resample Quality",
            "type": { "default": 4, "min": 0, "max": 14 },
            "params": true
          },
          {
            "name": "resample.disable",
            "description": "Disable Resampling",
            "type": {
              "default": false,
              "alt1": false,
              "alt2": true
            },
            "params": true
          },
          {
            "name": "dither.noise",
            "description": "Add noise bits",
            "type": { "default": 0, "min": 0, "max": 16 },
            "params": true
          },
          {
            "name": "dither.method",
            "description": "The dithering method",
            "type": "none",
            "params": true,
            "labels": [
              "none",
              "Disabled",
              "rectangular",
              "Rectangular dithering",
              "triangular",
              "Triangular dithering",
              "triangular-hf",
              "Sloped Triangular dithering",
              "wannamaker3",
              "Wannamaker 3 dithering",
              "shaped5",
              "Lipshitz 5 dithering"
            ]
          },
          {
            "name": "debug.wav-path",
            "description": "Path to WAV file",
            "type": "",
            "params": true
          },
          {
            "name": "channelmix.lock-volumes",
            "description": "Disable volume updates",
            "type": {
              "default": false,
              "alt1": false,
              "alt2": true
            },
            "params": true
          },
          {
            "name": "audioconvert.filter-graph.disable",
            "description": "Disable Filter graph updates",
            "type": {
              "default": false,
              "alt1": false,
              "alt2": true
            },
            "params": true
          },
          {
            "name": "audioconvert.filter-graph.N",
            "description": "A filter graph to load",
            "type": "",
            "params": true
          }
        ],
        "Props": [
          {
            "volume": 1.000000,
            "mute": false,
            "channelVolumes": [ 1.000000, 1.000000 ],
            "channelMap": [ "FL", "FR" ],
            "softMute": false,
            "softVolumes": [ 1.000000, 1.000000 ],
            "monitorMute": false,
            "monitorVolumes": [ 1.000000, 1.000000 ],
            "params": [
              "monitor.channel-volumes",
              false,
              "channelmix.disable",
              false,
              "channelmix.min-volume",
              0.000000,
              "channelmix.max-volume",
              10.000000,
              "channelmix.normalize",
              false,
              "channelmix.mix-lfe",
              true,
              "channelmix.upmix",
              true,
              "channelmix.lfe-cutoff",
              0.000000,
              "channelmix.fc-cutoff",
              0.000000,
              "channelmix.rear-delay",
              0.000000,
              "channelmix.stereo-widen",
              0.000000,
              "channelmix.hilbert-taps",
              0,
              "channelmix.upmix-method",
              "none",
              "resample.quality",
              4,
              "resample.disable",
              false,
              "dither.noise",
              0,
              "dither.method",
              "none",
              "debug.wav-path",
              "",
              "channelmix.lock-volumes",
              false,
              "audioconvert.filter-graph.disable",
              false,
              "audioconvert.filter-graph",
              ""
            ]
          }
        ],
        "Format": [
          {
            "mediaType": "audio",
            "mediaSubtype": "raw",
            "format": "F32LE",
            "rate": 48000,
            "channels": 2,
            "position": [ "FL", "FR" ]
          }
        ],
        "EnumPortConfig": [
          {
            "direction": "Output",
            "mode": {
              "default": "none",
              "alt1": "none",
              "alt2": "dsp",
              "alt3": "convert"
            },
            "monitor": {
              "default": false,
              "alt1": false,
              "alt2": true
            },
            "control": {
              "default": false,
              "alt1": false,
              "alt2": true
            }
          }
        ],
        "PortConfig": [
          {
            "direction": "Output",
            "mode": "dsp",
            "monitor": true,
            "control": false,
            "format": {
              "mediaType": "audio",
              "mediaSubtype": "raw",
              "format": "F32P",
              "channels": 2,
              "position": [ "FL", "FR" ]
            }
          }
        ],
        "Latency": [
          {
            "direction": "Input",
            "minQuantum": 1.000000,
            "maxQuantum": 1.000000,
            "minRate": 0,
            "maxRate": 0,
            "minNs": 0,
            "maxNs": 0
          }
        ],
        "ProcessLatency": [
        ],
        "Tag": [
          {
            "direction": "Input"
          }
        ]
      }
    }
  }
*/