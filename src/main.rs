
//! Full-featured Rust CLI for wireless ADB phone control with audio routing and camera streaming
//! Optimized for Linux with pattern unlock support and live camera streaming
//! By Sanjai Shaarugesh - https://github.com/Sanjai-Shaarugesh/phonectl

use phonectl::{
    adb::{ensure_adb_connected, show_about, setup_wizard, switch_device, rename_device, reconnect_saved_device, config_install_mode},
    audio::handle_audio,
    camera::handle_camera,
    contacts::{call_prompt, dial_prompt, list_contacts, search_prompt, answer_call, end_call},
    unlock::{configure_unlock, wake_phone, unlock_phone},
};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_help();
        return;
    }

    let cmd = args[1].as_str();

    match cmd {
        "setup" => setup_wizard(),
        "config" => configure_unlock(),
        "unlock" => ensure_adb_connected(unlock_phone),
        "reconnect" => reconnect_saved_device(),
        "switch-device" => switch_device(),
        "rename-device" => {
            if args.len() >= 4 {
                rename_device(&args[2], &args[3]);
            } else {
                println!("Usage: phonectl rename-device <ip> <new_name>");
            }
        },
        "config-install" => {
            if args.len() >= 3 {
                config_install_mode(&args[2]);
            } else {
                println!("Usage: phonectl config-install <auto|manual>");
            }
        },
        "list" => ensure_adb_connected(list_contacts),
        "search" => {
            if args.len() >= 3 {
                ensure_adb_connected(|| search_prompt(&args[2..].join(" ")));
            } else {
                println!("Usage: phonectl search <name|number>");
            }
        }
        "call" => {
            if args.len() >= 3 {
                ensure_adb_connected(|| call_prompt(&args[2..].join(" ")));
            } else {
                println!("Usage: phonectl call <name|number>");
            }
        }
        "dial" => {
            if args.len() >= 3 {
                ensure_adb_connected(|| dial_prompt(&args[2..].join(" ")));
            } else {
                println!("Usage: phonectl dial <name|number>");
            }
        }
        "answer" => ensure_adb_connected(answer_call),
        "reject" | "end" => ensure_adb_connected(end_call),
        "wake" => ensure_adb_connected(wake_phone),
        "audio" => {
            if args.len() >= 3 {
                ensure_adb_connected(|| handle_audio(&args[2]));
            } else {
                println!("Usage: phonectl audio <start|stop|status>");
            }
        }
        "camera" => {
            if args.len() >= 3 {
                let camera_args: Vec<String> = args[3..].to_vec();
                ensure_adb_connected(|| handle_camera(&args[2], &camera_args));
            } else {
                println!("Usage: phonectl camera <start|stop|record|capture|status|install>");
            }
        }
        "about" => show_about(),
        _ => print_help(),
    }
}

fn print_help() {
    println!("📱 Rust ADB Phone Control CLI v2.0");
    println!("===========================================");
    println!("📋 Setup Commands:");
    println!("  setup                     - Complete setup wizard for new device");
    println!("  config                    - Setup and store unlock PIN/pattern (encrypted)");
    println!("  config-install <mode>     - Set installation mode (auto/manual)");
    println!("  reconnect                 - Reconnect to previously saved devices");
    println!("  switch-device             - Switch between saved devices");
    println!("  rename-device <ip> <name> - Rename a saved device");
    println!("");
    println!("🔓 Phone Control:");
    println!("  unlock                    - Unlock the phone using saved PIN/pattern");
    println!("  wake                      - Wake up the phone screen");
    println!("");
    println!("📞 Contact & Call Management:");
    println!("  list                      - List all contacts");
    println!("  search <query>            - Search contact by name or number");
    println!("  call <name|number>        - Call a contact or number");
    println!("  dial <name|number>        - Open dialer for a contact or number");
    println!("  answer                    - Answer incoming call");
    println!("  reject / end              - End or reject call");
    println!("");
    println!("🎵 Audio Management:");
    println!("  audio start               - Start call audio routing");
    println!("  audio stop                - Stop call audio routing");
    println!("  audio status              - Check audio routing status");
    println!("");
    println!("📹 Camera & Streaming:");
    println!("  camera start              - Start live camera stream");
    println!("  camera stop               - Stop camera stream");
    println!("  camera capture            - Take a photo");
    println!("  camera record [seconds]   - Record video (default: 30s)");
    println!("  camera status             - Check camera status");
    println!("  camera install            - Install IP Webcam APK");
    println!("");
    println!("ℹ️  Information:");
    println!("  about                     - Show developer details");
    println!("");
    println!("🚀 Quick Start:");
    println!("  1. Connect phone via USB and run: phonectl setup");
    println!("  2. Configure unlock: phonectl config");
    println!("  3. Start camera stream: phonectl camera start");
    println!("");
    println!("📦 Dependencies (auto-installed on supported distros):");
    println!("  • android-tools (adb) • sox • alsa-utils • nmap-ncat • ffmpeg");
}