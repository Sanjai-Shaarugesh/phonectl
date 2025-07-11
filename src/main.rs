//! Full-featured Rust CLI for wireless ADB phone control with audio routing
//! Optimized for Fedora Linux with pattern unlock support
//! By Sanjai Shaarugesh - https://github.com/Sanjai-Shaarugesh

use phonectl::{
    adb::{ensure_adb_connected, show_about, setup_wizard},
    audio::handle_audio,
    contacts::{call_prompt, dial_prompt, list_contacts, search_prompt},
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
        "reconnect" => phonectl::adb::reconnect_saved_device(),
        "list" => ensure_adb_connected(list_contacts),
        "search" => {
            if args.len() >= 3 {
                ensure_adb_connected(|| search_prompt(&args[2..].join(" ")));
            } else {
                println!("❌ Usage: phonectl search <name|number>");
            }
        }
        "call" => {
            if args.len() >= 3 {
                ensure_adb_connected(|| call_prompt(&args[2..].join(" ")));
            } else {
                println!("❌ Usage: phonectl call <name|number>");
            }
        }
        "dial" => {
            if args.len() >= 3 {
                ensure_adb_connected(|| dial_prompt(&args[2..].join(" ")));
            } else {
                println!("❌ Usage: phonectl dial <name|number>");
            }
        }
        "answer" => ensure_adb_connected(phonectl::contacts::answer_call),
        "reject" | "end" => ensure_adb_connected(phonectl::contacts::end_call),
        "wake" => ensure_adb_connected(wake_phone),
        "audio" => {
            if args.len() >= 3 {
                ensure_adb_connected(|| handle_audio(&args[2]));
            } else {
                println!("Usage: phonectl audio <start|stop|status>");
            }
        }
        "about" => show_about(),
        _ => print_help(),
    }
}

fn print_help() {
    println!("📱 Rust ADB Phone Control CLI v1.5");
    println!("===========================================");
    println!("Commands:");
    println!("  setup                     - Complete setup wizard for new device");
    println!("  config                    - Setup and store unlock PIN/pattern (encrypted)");
    println!("  unlock                    - Unlock the phone using saved PIN/pattern");
    println!("  reconnect                 - Reconnect to previously saved devices");
    println!("  list                      - List all contacts");
    println!("  search <query>            - Search contact by name or number");
    println!("  call <name|number>        - Call a contact or number");
    println!("  dial <name|number>        - Open dialer for a contact or number");
    println!("  answer                    - Answer incoming call");
    println!("  reject / end              - End or reject call");
    println!("  wake                      - Wake up the phone screen");
    println!("  audio <start|stop|status> - Manage call audio routing");
    println!("  about                     - Show developer details");
    println!("");
    println!("First time setup: Connect phone via USB and run 'phonectl setup'");
    println!("Fedora dependencies: sudo dnf install android-tools sox alsa-utils nmap-ncat");

}