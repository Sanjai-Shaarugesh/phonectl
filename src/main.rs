//! Full-featured Rust CLI for wireless ADB setup, contact search, calling, phone unlock with persistence
//! Includes animations, PIN/password storage with encryption, and reconnection to previously connected phones

use aes_gcm::aead::Aead;
use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::{Aes256Gcm, KeyInit};
use base64;
use dirs;
use rand::Rng;
use regex::Regex;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[derive(Clone, Debug)]
struct Contact {
    name: String,
    number: String,
}

const CONFIG_FILE: &str = ".phonectl_auth";
const DEVICE_FILE: &str = ".phonectl_devices";
const KEY_FILE: &str = ".phonectl_key";

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_help();
        return;
    }

    match args[1].as_str() {
        "setup" => setup_wizard(),
        "config" => configure_unlock(),
        "unlock" => ensure_adb_connected(unlock_phone),
        "reconnect" => reconnect_saved_device(),
        "list" => ensure_adb_connected(list_contacts),
        "search" if args.len() >= 3 => ensure_adb_connected(|| search_prompt(&args[2..].join(" "))),
        "call" if args.len() >= 3 => ensure_adb_connected(|| call_prompt(&args[2..].join(" "))),
        "dial" if args.len() >= 3 => ensure_adb_connected(|| dial_prompt(&args[2..].join(" "))),
        "answer" => {
            ensure_adb_connected(|| adb(&["shell", "input", "swipe", "500", "1600", "500", "1000"]))
        }
        "reject" | "end" => {
            ensure_adb_connected(|| adb(&["shell", "input", "keyevent", "KEYCODE_ENDCALL"]))
        }
        "wake" => ensure_adb_connected(wake_phone),
        "about" => show_about(),
        _ => print_help(),
    }
}

fn print_help() {
    println!("📱 Rust ADB Phone Control CLI v1.1");
    println!("=================================");
    println!("Commands:");
    println!("  setup                     - Complete setup wizard for new device");
    println!("  config                    - Setup and store unlock PIN/password (encrypted)");
    println!("  unlock                    - Unlock the phone using saved PIN/password");
    println!("  reconnect                 - Reconnect to previously saved devices");
    println!("  list                      - List all contacts");
    println!("  search <query>           - Search contact by name or number");
    println!("  call <name|number>       - Call a contact or number");
    println!("  dial <name|number>       - Open dialer for a contact or number");
    println!("  answer                   - Answer incoming call");
    println!("  reject / end             - End or reject call");
    println!("  wake                     - Wake up the phone screen");
    println!("  about                    - Show developer details");
    println!("");
    println!("First time setup: Connect phone via USB and run 'phonectl setup'");
}

fn show_about() {
    println!("📱 Wireless ADB Phone Control CLI");
    println!("==================================");
    println!("Version: 1.1");
    println!("Developer: Sanjai Shaarugesh");
    println!("GitHub: https://github.com/Sanjai-Shaarugesh/phonectl");
    println!("Description: Full-featured Rust CLI for wireless ADB control");
    println!("Features:");
    println!("  • Encrypted credential storage");
    println!("  • Automatic device reconnection");
    println!("  • Contact management");
    println!("  • Call/SMS functionality");
    println!("  • Phone unlock automation");
    println!("");
    println!("License: MIT");
    println!("Built with ❤️ in Rust");
}

fn get_config_path() -> PathBuf {
    dirs::home_dir().unwrap().join(CONFIG_FILE)
}

fn get_device_file_path() -> PathBuf {
    dirs::home_dir().unwrap().join(DEVICE_FILE)
}

fn get_key_file_path() -> PathBuf {
    dirs::home_dir().unwrap().join(KEY_FILE)
}

fn generate_or_get_key() -> [u8; 32] {
    let key_path = get_key_file_path();

    if key_path.exists() {
        let encoded = fs::read_to_string(key_path).unwrap();
        let decoded = base64::decode_config(encoded.trim(), base64::STANDARD_NO_PAD).unwrap();
        let mut key = [0u8; 32];
        key.copy_from_slice(&decoded);
        key
    } else {
        let mut rng = rand::rng();
        let key: [u8; 32] = rng.random();
        let encoded = base64::encode_config(key, base64::STANDARD_NO_PAD);
        fs::write(key_path, encoded).unwrap();
        key
    }
}

fn encrypt_data(data: &str) -> String {
    let key = generate_or_get_key();
    let key_array = GenericArray::from_slice(&key);
    let cipher = Aes256Gcm::new(key_array);
    let nonce_bytes: [u8; 12] = rand::rng().random();
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, data.as_bytes()).unwrap();
    let mut result = nonce_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    base64::encode_config(result, base64::STANDARD_NO_PAD)
}

fn decrypt_data(encrypted: &str) -> Result<String, String> {
    let encrypted = encrypted.trim();
    let key = generate_or_get_key();
    let key_array = GenericArray::from_slice(&key);
    let cipher = Aes256Gcm::new(key_array);

    let data = base64::decode_config(encrypted, base64::STANDARD_NO_PAD)
        .map_err(|e| format!("Base64 decode error: {}", e))?;
    if data.len() < 12 {
        return Err("Invalid encrypted data".to_string());
    }

    let nonce = GenericArray::from_slice(&data[..12]);
    let ciphertext = &data[12..];

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption error: {}", e))?;
    String::from_utf8(plaintext).map_err(|e| format!("UTF8 error: {}", e))
}

fn setup_wizard() {
    println!("🚀 Welcome to ADB Wireless Setup Wizard!");
    println!("========================================");
    println!("");

    // Step 1: Check ADB
    println!("Step 1: Checking ADB installation...");
    if !check_adb_installed() {
        println!("❌ ADB not found. Please install Android SDK Platform Tools first.");
        println!("Download from: https://developer.android.com/studio/releases/platform-tools");
        return;
    }
    println!("✅ ADB is installed and ready");

    // Step 2: USB Connection Check
    println!("");
    println!("Step 2: Connect your phone via USB cable");
    println!("📱 Please ensure:");
    println!("  • USB cable is connected to your phone and computer");
    println!("  • USB Debugging is enabled in Developer Options");
    println!("  • Phone is unlocked and you've allowed USB debugging");
    println!("");
    print!("Press Enter when phone is connected via USB...");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    // Step 3: Check USB connection
    println!("Step 3: Checking USB connection...");
    let devices = get_usb_devices();
    if devices.is_empty() {
        println!("❌ Phone not detected via USB. Please check:");
        println!("  • USB cable is properly connected");
        println!("  • USB Debugging is enabled");
        println!("  • You've authorized this computer on your phone");
        return;
    }

    let device_id = if devices.len() == 1 {
        devices.keys().next().unwrap().clone()
    } else {
        println!("Multiple devices detected:");
        for (i, (id, model)) in devices.iter().enumerate() {
            println!("{}. {} ({})", i + 1, model, id);
        }
        print!("Select the device number to set up: ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let idx: usize = input.trim().parse().unwrap_or(0);
        if idx == 0 || idx > devices.len() {
            println!("⚠️ Invalid selection, using first device.");
            devices.keys().next().unwrap().clone()
        } else {
            devices.keys().nth(idx - 1).unwrap().clone()
        }
    };

    println!("✅ Phone detected via USB: {}", devices[&device_id]);

    // Step 4: Enable TCP/IP mode
    println!("");
    println!("Step 4: Enabling ADB over Wi-Fi...");
    animation_spinner("Configuring ADB TCP/IP mode");
    let status = Command::new("adb")
        .args(&["-s", &device_id, "tcpip", "5555"])
        .status()
        .expect("Failed to run adb tcpip");

    if !status.success() {
        println!("❌ Failed to enable ADB over Wi-Fi. Please check your device connection.");
        return;
    }
    println!("✅ ADB TCP/IP mode enabled");
    sleep(Duration::from_secs(2));

    // Step 5: Get device IP
    println!("Step 5: Detecting device IP address...");
    let ip = get_device_ip(&device_id);
    if ip.is_empty() {
        println!("❌ Could not detect device IP. Please ensure:");
        println!("  • Phone is connected to Wi-Fi");
        println!("  • Phone and computer are on the same network");
        return;
    }
    println!("✅ Device IP detected: {}", ip);

    // Step 6: Connect wirelessly
    println!("");
    println!("Step 6: Connecting wirelessly...");
    animation_spinner("Establishing wireless connection");
    let status = Command::new("adb")
        .args(&["connect", &format!("{}:5555", ip)])
        .status()
        .expect("Failed to run adb connect");

    if !status.success() {
        println!("❌ Wireless connection failed. Please retry setup.");
        return;
    }

    // Step 7: Save device
    save_device_ip(&ip);

    // Step 8: Test connection
    println!("Step 7: Testing wireless connection...");
    if !is_adb_connected() {
        println!("❌ Wireless connection test failed.");
        return;
    }

    println!("✅ Wireless connection established successfully!");
    println!("");
    println!("🎉 Setup Complete!");
    println!("=================");
    println!("You can now disconnect the USB cable.");
    println!("Your device will automatically reconnect when both devices are on.");
    println!("");
    println!("Next steps:");
    println!("  • Run 'phonectl config' to set up phone unlock");
    println!("  • Run 'phonectl list' to see your contacts");
    println!("  • Run 'phonectl reconnect' to reconnect saved devices");
}

fn check_adb_installed() -> bool {
    Command::new("adb")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn get_usb_devices() -> HashMap<String, String> {
    let _output = Command::new("adb")
        .args(&["devices", "-l"])
        .output()
        .expect("Failed to run adb devices");

    let mut devices = HashMap::new();
    let out = String::from_utf8_lossy(&_output.stdout);

    for line in out.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 || parts[1] != "device" {
            continue;
        }

        let device_id = parts[0].to_string();
        let model = parts
            .iter()
            .find(|p| p.starts_with("model:"))
            .map(|s| s.split(':').nth(1).unwrap_or("Unknown"))
            .unwrap_or("Unknown")
            .to_string();

        devices.insert(device_id, model);
    }

    devices
}

fn get_device_ip(device_id: &str) -> String {
    let _output = Command::new("adb")
        .args(&["-s", device_id, "shell", "ip", "addr", "show", "wlan0"])
        .output()
        .expect("Failed to get IP address");

    let stdout = String::from_utf8_lossy(&_output.stdout);
    let re = Regex::new(r"inet (\d+\.\d+\.\d+\.\d+)").unwrap();

    re.captures_iter(&stdout)
        .next()
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

fn save_device_ip(ip: &str) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(get_device_file_path())
        .expect("Unable to store device IP");
    writeln!(file, "{}:5555", ip).ok();
}

fn configure_unlock() {
    println!("🔐 Configure Phone Unlock");
    println!("=========================");
    print!("Enter your phone unlock PIN/password: ");
    io::stdout().flush().unwrap();
    let mut pin = String::new();
    io::stdin().read_line(&mut pin).unwrap();
    let pin = pin.trim();

    if pin.is_empty() {
        println!("❌ Unlock code cannot be empty.");
        return;
    }

    let encrypted = encrypt_data(pin);
    fs::write(get_config_path(), encrypted).expect("Failed to write unlock config");
    println!("✅ Unlock PIN/password saved successfully (encrypted).");
    println!("🔒 Your credentials are stored encrypted on disk.");
}

fn reconnect_saved_device() {
    println!("🔄 Reconnecting to saved devices...");
    let path = get_device_file_path();
    if !path.exists() {
        println!("❌ No saved devices found.");
        println!("💡 Run 'phonectl setup' to configure a new device.");
        return;
    }

    let content = fs::read_to_string(path).unwrap_or_default();
    let ips: HashSet<String> = content.lines().map(|s| s.trim().to_string()).collect();

    if ips.is_empty() {
        println!("❌ No saved devices to reconnect.");
        return;
    }

    println!("Found {} saved device(s):", ips.len());
    for ip in &ips {
        println!("  • {}", ip);
    }
    println!("");

    let mut connected = false;
    for ip in &ips {
        println!("🔄 Connecting to {}...", ip);
        let output = Command::new("adb")
            .args(&["connect", ip])
            .output()
            .expect("Failed to connect");

        sleep(Duration::from_millis(500));

        if is_adb_connected() {
            println!("✅ Successfully connected to {}", ip);
            connected = true;
            break;
        } else {
            println!("❌ Failed to connect to {}", ip);
        }
    }

    if connected {
        println!("🎉 Device reconnected successfully!");
    } else {
        println!("❌ Could not reconnect to any saved device.");
        println!("💡 Make sure your phone is on the same Wi-Fi network and try again.");
    }
}

fn unlock_phone() {
    println!("🔓 Unlocking phone...");
    let path = get_config_path();
    if !path.exists() {
        println!("⚠️  No unlock PIN/password saved. Run 'phonectl config' first.");
        return;
    }

    let encrypted = fs::read_to_string(path).unwrap();
    let pin = match decrypt_data(&encrypted) {
        Ok(p) => p,
        Err(e) => {
            println!("❌ Failed to decrypt unlock code: {}", e);
            println!("💡 Run 'phonectl config' to reconfigure.");
            return;
        }
    };

    animation_spinner("Unlocking phone");

    // Wake up the screen
    adb(&["shell", "input", "keyevent", "KEYCODE_WAKEUP"]);
    sleep(Duration::from_millis(500));

    // Swipe up to unlock (for swipe unlock screens)
    adb(&["shell", "input", "swipe", "500", "1000", "500", "500"]);
    sleep(Duration::from_millis(500));

    // Enter PIN/password
    adb(&["shell", "input", "text", &pin.trim().replace(" ", "")]);
    sleep(Duration::from_millis(300));

    // Press Enter
    adb(&["shell", "input", "keyevent", "KEYCODE_ENTER"]);
    sleep(Duration::from_millis(500));

    // Keep screen awake
    adb(&["shell", "svc", "power", "stayon", "true"]);

    println!("✅ Phone unlocked and screen will stay awake");
}

fn wake_phone() {
    println!("☀️ Waking up phone...");
    adb(&["shell", "input", "keyevent", "KEYCODE_WAKEUP"]);
    adb(&["shell", "svc", "power", "stayon", "true"]);
    println!("✅ Phone screen is now awake");
}

fn animation_spinner(label: &str) {
    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    for i in 0..30 {
        print!("\r{} {}...", spinner[i % spinner.len()], label);
        io::stdout().flush().unwrap();
        sleep(Duration::from_millis(100));
    }
    println!("\r✅ {}", label);
}

fn adb(args: &[&str]) {
    let status = Command::new("adb").args(args).status().unwrap_or_else(|_| {
        eprintln!("❌ Failed to execute ADB command: {:?}", args);
        std::process::exit(1);
    });

    if !status.success() {
        eprintln!("❌ ADB command failed: {:?}", args);
    }
}

fn is_adb_connected() -> bool {
    let output = Command::new("adb")
        .arg("devices")
        .output()
        .expect("Failed to run adb devices");

    let out = String::from_utf8_lossy(&output.stdout);
    out.lines()
        .any(|line| line.contains("device") && !line.contains("List"))
}

fn ensure_adb_connected<F: FnOnce()>(func: F) {
    if !is_adb_connected() {
        println!("🔌 ADB not connected. Attempting to reconnect...");
        reconnect_saved_device();
    }

    if is_adb_connected() {
        func();
    } else {
        println!("❌ ADB connection failed. Please run 'phonectl reconnect' or 'phonectl setup'.");
    }
}

fn get_contacts() -> Vec<Contact> {
    let output = Command::new("adb")
        .args(&[
            "shell",
            "content",
            "query",
            "--uri",
            "content://com.android.contacts/data/phones",
            "--projection",
            "display_name:data1",
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
    println!("📞 Contact List");
    println!("===============");
    let contacts = get_contacts();

    if contacts.is_empty() {
        println!("No contacts found.");
        return;
    }

    for (i, c) in contacts.iter().enumerate() {
        println!("{}. {} → {}", i + 1, c.name, c.number);
    }
    println!("\nTotal: {} contacts", contacts.len());
}

fn search_prompt(query: &str) {
    println!("🔍 Searching for: {}", query);
    let results: Vec<_> = get_contacts()
        .into_iter()
        .filter(|c| {
            c.name.to_lowercase().contains(&query.to_lowercase()) || c.number.contains(query)
        })
        .collect();

    if results.is_empty() {
        println!("❌ No contacts found for '{}'", query);
        println!("💡 Try a different search term or check spelling.");
        return;
    }

    println!("Found {} match(es):", results.len());
    for (i, c) in results.iter().enumerate() {
        println!("{}. {} → {}", i + 1, c.name, c.number);
    }

    print!("\nWould you like to call one of these contacts? (y/n): ");
    io::stdout().flush().unwrap();
    let mut ans = String::new();
    io::stdin().read_line(&mut ans).unwrap();
    if ans.trim().eq_ignore_ascii_case("y") {
        prompt_and_exec(results, "call");
    }
}

fn prompt_and_exec(filtered: Vec<Contact>, action: &str) {
    if filtered.is_empty() {
        println!("❌ No matches found. Would you like to search instead? (y/n): ");
        io::stdout().flush().unwrap();
        let mut s = String::new();
        io::stdin().read_line(&mut s).unwrap();
        if s.trim().eq_ignore_ascii_case("y") {
            print!("🔍 Enter search query: ");
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
        println!("Multiple matches found:");
        for (i, c) in filtered.iter().enumerate() {
            println!("{}. {} → {}", i + 1, c.name, c.number);
        }
        print!("Select contact [1-{}]: ", filtered.len());
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let idx = input.trim().parse::<usize>().unwrap_or(0);
        if idx == 0 || idx > filtered.len() {
            println!("❌ Invalid selection");
            return;
        }
        filtered[idx - 1].clone()
    };

    let intent = if action == "call" { "CALL" } else { "DIAL" };
    println!("📞 {}ing {} ({})", action, selected.name, selected.number);
    adb(&[
        "shell",
        "am",
        "start",
        "-a",
        &format!("android.intent.action.{}", intent),
        "-d",
        &format!("tel:{}", selected.number),
    ]);
}

fn call_prompt(query: &str) {
    let contacts = get_contacts();
    let filtered = if query.chars().all(|c| c.is_ascii_digit() || c == '+') {
        vec![Contact {
            name: "Direct Number".into(),
            number: query.into(),
        }]
    } else {
        contacts
            .into_iter()
            .filter(|c| {
                c.name.to_lowercase().contains(&query.to_lowercase()) || c.number.contains(query)
            })
            .collect()
    };
    prompt_and_exec(filtered, "call");
}

fn dial_prompt(query: &str) {
    let contacts = get_contacts();
    let filtered = if query.chars().all(|c| c.is_ascii_digit() || c == '+') {
        vec![Contact {
            name: "Direct Number".into(),
            number: query.into(),
        }]
    } else {
        contacts
            .into_iter()
            .filter(|c| {
                c.name.to_lowercase().contains(&query.to_lowercase()) || c.number.contains(query)
            })
            .collect()
    };
    prompt_and_exec(filtered, "dial");
}
