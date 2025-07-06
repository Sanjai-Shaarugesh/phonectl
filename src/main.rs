//! Full-featured Rust CLI for wireless ADB setup, contact search, calling, phone unlock with persistence
//! Includes animations, PIN/password storage, and reconnection to previously connected phones

use std::io::{self, Write};
use std::process::{Command};
use std::env;
use std::thread::sleep;
use std::time::Duration;
use regex::Regex;
use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::io::Read;
use std::collections::HashSet;
use dirs;

#[derive(Clone, Debug)]
struct Contact {
    name: String,
    number: String,
}

const CONFIG_FILE: &str = ".phonectl_auth";
const DEVICE_FILE: &str = ".phonectl_devices";

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_help();
        return;
    }

    match args[1].as_str() {
        "setup" => setup_adb_wifi(),
        "config" => configure_unlock(),
        "unlock" => ensure_adb_connected(unlock_phone),
        "reconnect" => reconnect_saved_device(),
        "list" => ensure_adb_connected(list_contacts),
        "search" if args.len() >= 3 => ensure_adb_connected(|| search_prompt(&args[2..].join(" "))),
        "call" if args.len() >= 3 => ensure_adb_connected(|| call_prompt(&args[2..].join(" "))),
        "dial" if args.len() >= 3 => ensure_adb_connected(|| dial_prompt(&args[2..].join(" "))),
        "answer" => ensure_adb_connected(|| adb(&["shell", "input", "swipe", "500", "1600", "500", "1000"])),
        "reject" | "end" => ensure_adb_connected(|| adb(&["shell", "input", "keyevent", "KEYCODE_ENDCALL"])),
        _ => print_help(),
    }
}

fn print_help() {
    println!("Rust ADB Phone Control CLI");
    println!("Commands:");
    println!("  setup                     - Setup and connect ADB over Wi-Fi");
    println!("  config                    - Setup and store unlock PIN/password");
    println!("  unlock                    - Unlock the phone using saved PIN/password");
    println!("  reconnect                 - Reconnect to a previously saved device IP");
    println!("  list                      - List all contacts");
    println!("  search <query>           - Search contact by name or number");
    println!("  call <name|number>       - Call a contact or number");
    println!("  dial <name|number>       - Open dialer for a contact or number");
    println!("  answer                   - Answer incoming call");
    println!("  reject / end             - End or reject call");
}

fn get_config_path() -> PathBuf {
    dirs::home_dir().unwrap().join(CONFIG_FILE)
}

fn get_device_file_path() -> PathBuf {
    dirs::home_dir().unwrap().join(DEVICE_FILE)
}

fn configure_unlock() {
    print!("Enter your phone unlock PIN/password: ");
    io::stdout().flush().unwrap();
    let mut pin = String::new();
    io::stdin().read_line(&mut pin).unwrap();
    let pin = pin.trim();

    if pin.is_empty() {
        println!("❌ Unlock code cannot be empty.");
        return;
    }

    fs::write(get_config_path(), pin).expect("Failed to write unlock config");
    println!("✅ Unlock PIN/password saved successfully.");
}

fn reconnect_saved_device() {
    let path = get_device_file_path();
    if !path.exists() {
        println!("❌ No saved devices to reconnect.");
        return;
    }

    let content = fs::read_to_string(path).unwrap_or_default();
    let ips: HashSet<String> = content.lines().map(|s| s.trim().to_string()).collect();

    for ip in &ips {
        println!("🔄 Reconnecting to {ip}...");
        adb(&["connect", ip]);
        sleep(Duration::from_millis(300));
    }

    println!("✅ Reconnection attempts complete.");
}

fn unlock_phone() {
    println!("🔓 Unlocking phone using stored credentials...");
    let path = get_config_path();
    if !path.exists() {
        println!("⚠️ No unlock PIN/password saved. Run `phonectl config` first.");
        return;
    }

    let mut pin = String::new();
    File::open(path).unwrap().read_to_string(&mut pin).unwrap();
    adb(&["shell", "input", "keyevent", "82"]);
    sleep(Duration::from_millis(500));
    adb(&["shell", "input", "text", &pin.trim().replace(" ", "")]);
    sleep(Duration::from_millis(300));
    adb(&["shell", "input", "keyevent", "66"]);
    adb(&["shell", "svc", "power", "stayon", "true"]);
    animation_spinner("Phone Unlocked and Awake");
}

fn animation_spinner(label: &str) {
    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    for i in 0..spinner.len() {
        print!("\r{} {}", spinner[i], label);
        io::stdout().flush().unwrap();
        sleep(Duration::from_millis(100));
    }
    println!("\r✅ {}", label);
}

fn adb(args: &[&str]) {
    let status = Command::new("adb")
        .args(args)
        .status()
        .expect("Failed to run adb");

    if !status.success() {
        eprintln!("ADB command failed: {:?}", args);
    }
}

fn is_adb_connected() -> bool {
    let output = Command::new("adb")
        .arg("devices")
        .output()
        .expect("Failed to run adb devices");

    let out = String::from_utf8_lossy(&output.stdout);
    out.lines().any(|line| line.contains("device") && !line.contains("List"))
}

fn ensure_adb_connected<F: FnOnce()>(func: F) {
    if !is_adb_connected() {
        println!("🔌 ADB not connected. Attempting setup...");
        setup_adb_wifi();
    }

    if is_adb_connected() {
        func();
    } else {
        println!("❌ ADB connection failed. Please connect manually with 'phonectl setup'.");
    }
}

fn setup_adb_wifi() {
    println!("🔌 Enabling ADB over Wi-Fi");
    adb(&["tcpip", "5555"]);

    let output = Command::new("adb")
        .args(&["shell", "ip", "route"])
        .output()
        .expect("Failed to get IP route");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let ip = stdout
        .lines()
        .find(|line| line.contains("src"))
        .and_then(|line| line.split("src ").nth(1))
        .map(|s| s.trim().split_whitespace().next().unwrap_or(""))
        .unwrap_or("");

    if ip.is_empty() {
        println!("⚠️ Could not detect device IP.");
        return;
    }

    println!("📡 Connecting to ADB over Wi-Fi at {ip}");
    animation_spinner("Connecting to device");
    adb(&["connect", ip]);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(get_device_file_path())
        .expect("Unable to store device IP");
    writeln!(file, "{}", ip).ok();

    println!("✅ Connected. You can now unplug the USB.");
}

fn get_contacts() -> Vec<Contact> {
    let output = Command::new("adb")
        .args(&[
            "shell", "content", "query",
            "--uri", "content://com.android.contacts/data/phones",
            "--projection", "display_name:data1"
        ])
        .output()
        .expect("Failed to query contacts");

    let data = String::from_utf8_lossy(&output.stdout);
    let re = Regex::new(r"display_name=(.*), data1=(.*)").unwrap();

    re.captures_iter(&data)
        .map(|c| Contact {
            name: c[1].trim().to_string(),
            number: c[2].trim().to_string(),
        })
        .collect()
}

fn list_contacts() {
    for (i, c) in get_contacts().iter().enumerate() {
        println!("{}. {} → {}", i + 1, c.name, c.number);
    }
}

fn search_prompt(query: &str) {
    let results: Vec<_> = get_contacts()
        .into_iter()
        .filter(|c| c.name.to_lowercase().contains(&query.to_lowercase()) || c.number.contains(query))
        .collect();

    if results.is_empty() {
        println!("No contacts found. Try rephrasing or checking spelling.");
        return;
    }

    for (i, c) in results.iter().enumerate() {
        println!("{}. {} → {}", i + 1, c.name, c.number);
    }

    println!("\nWould you like to call one of these contacts? (y/n): ");
    io::stdout().flush().unwrap();
    let mut ans = String::new();
    io::stdin().read_line(&mut ans).unwrap();
    if ans.trim().eq_ignore_ascii_case("y") {
        prompt_and_exec(results, "call");
    }
}

fn prompt_and_exec(filtered: Vec<Contact>, action: &str) {
    if filtered.is_empty() {
        println!("No match found. Would you like to search instead? (y/n)");
        let mut s = String::new();
        io::stdin().read_line(&mut s).unwrap();
        if s.trim() == "y" {
            print!("Search query: ");
            io::stdout().flush().unwrap();
            let mut q = String::new();
            io::stdin().read_line(&mut q).unwrap();
            search_prompt(&q.trim());
        }
        return;
    }

    let selected = if filtered.len() == 1 {
        filtered[0].clone()
    } else {
        println!("Multiple matches:");
        for (i, c) in filtered.iter().enumerate() {
            println!("{}. {} → {}", i + 1, c.name, c.number);
        }
        print!("Select [1-{}]: ", filtered.len());
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let idx = input.trim().parse::<usize>().unwrap_or(0);
        if idx == 0 || idx > filtered.len() {
            println!("Invalid choice"); return;
        }
        filtered[idx - 1].clone()
    };

    let intent = if action == "call" { "CALL" } else { "DIAL" };
    adb(&["shell", "am", "start", "-a", &format!("android.intent.action.{}", intent), "-d", &format!("tel:{}", selected.number)]);
}

fn call_prompt(query: &str) {
    let contacts = get_contacts();
    let filtered = if query.chars().all(|c| c.is_ascii_digit()) {
        vec![Contact { name: "Direct".into(), number: query.into() }]
    } else {
        contacts.into_iter()
            .filter(|c| c.name.to_lowercase().contains(&query.to_lowercase()) || c.number.contains(query))
            .collect()
    };
    prompt_and_exec(filtered, "call");
}

fn dial_prompt(query: &str) {
    let contacts = get_contacts();
    let filtered = if query.chars().all(|c| c.is_ascii_digit()) {
        vec![Contact { name: "Direct".into(), number: query.into() }]
    } else {
        contacts.into_iter()
            .filter(|c| c.name.to_lowercase().contains(&query.to_lowercase()) || c.number.contains(query))
            .collect()
    };
    prompt_and_exec(filtered, "dial");
}
