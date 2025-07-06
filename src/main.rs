//! Full-featured Rust CLI for wireless ADB setup, contact search, and calling via Android phone

use std::io::{self, Write};
use std::process::Command;
use std::env;
use regex::Regex;

#[derive(Clone, Debug)]
struct Contact {
    name: String,
    number: String,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_help();
        return;
    }

    match args[1].as_str() {
        "setup" => setup_adb_wifi(),
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
    println!("  list                      - List all contacts");
    println!("  search <query>           - Search contact by name or number");
    println!("  call <name|number>       - Call a contact or number");
    println!("  dial <name|number>       - Open dialer for a contact or number");
    println!("  answer                   - Answer incoming call");
    println!("  reject / end             - End or reject call");
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
    adb(&["connect", ip]);
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
