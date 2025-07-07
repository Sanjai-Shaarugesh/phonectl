//! Full-featured Rust CLI for wireless ADB phone control with audio routing
//! Optimized for Fedora Linux with pattern unlock support
//! By Sanjai Shaarugesh - https://github.com/Sanjai-Shaarugesh

use aes_gcm::aead::Aead;
use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::{Aes256Gcm, KeyInit};
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use base64;
use dirs;
use rand::Rng;
use std::path::Path;
use base64::Engine;
use tempfile;
use reqwest;
use zip;
use std::fs::File;
use std::io::copy;

use regex::Regex;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, Write, Read};
use std::path::PathBuf;
use std::process::{Stdio};

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug)]
struct Contact {
    name: String,
    number: String,
}

const APK: &[u8] = include_bytes!("../assets/sndcpy.apk");
const CONFIG_FILE: &str = ".phonectl_auth";
const DEVICE_FILE: &str = ".phonectl_devices";
const KEY_FILE: &str = ".phonectl_key";
const AUDIO_FORWARD_PORT: &str = "28200";

static AUDIO_ACTIVE: AtomicBool = AtomicBool::new(false);



fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_help();
        return;
    }

     let _temp_path = Path::new("assets");

    let cmd = args[1].as_str();

    if cmd == "setup" {
        setup_wizard();
    } else if cmd == "config" {
        configure_unlock();
    } else if cmd == "unlock" {
        ensure_adb_connected(unlock_phone);
    } else if cmd == "reconnect" {
        reconnect_saved_device();
    } else if cmd == "list" {
        ensure_adb_connected(list_contacts);
    } else if cmd == "search" {
        if args.len() >= 3 {
            ensure_adb_connected(|| search_prompt(&args[2..].join(" ")));
        } else {
            println!("❌ Usage: phonectl search <name|number>");
        }
    } else if cmd == "call" {
        if args.len() >= 3 {
            ensure_adb_connected(|| call_prompt(&args[2..].join(" ")));
        } else {
            println!("❌ Usage: phonectl call <name|number>");
        }
    } else if cmd == "dial" {
        if args.len() >= 3 {
            ensure_adb_connected(|| dial_prompt(&args[2..].join(" ")));
        } else {
            println!("❌ Usage: phonectl dial <name|number>");
        }
    } else if cmd == "answer" {
        ensure_adb_connected(answer_call);
    } else if cmd == "reject" || cmd == "end" {
        ensure_adb_connected(end_call);
    } else if cmd == "wake" {
        ensure_adb_connected(wake_phone);
    } else if cmd == "audio" {
        if args.len() >= 3 {
            ensure_adb_connected(|| handle_audio(&args[2]));
        } else {
            println!("Usage: phonectl audio <start|stop|status>");
        }
    } else if cmd == "about" {
        show_about();
    } else {
        print_help();
    }
}

fn print_help() {
    println!("📱 Rust ADB Phone Control CLI v1.5 (Fedora)");
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

fn show_about() {
    println!("📱 Wireless ADB Phone Control CLI (Fedora)");
    println!("==========================================");
    println!("Version: 1.5");
    println!("Developer: Sanjai Shaarugesh");
    println!("GitHub: https://github.com/Sanjai-Shaarugesh/phonectl");
    println!("Description: Full-featured Rust CLI for wireless ADB control");
    println!("Optimized for Fedora Linux");
    println!("Features:");
    println!("  • Encrypted credential storage");
    println!("  • Pattern unlock support");
    println!("  • Automatic device reconnection");
    println!("  • Contact management");
    println!("  • Call functionality");
    println!("  • Phone unlock automation");
    println!("  • Full-duplex call audio routing");
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
        let encoded = fs::read_to_string(&key_path).unwrap_or_default();
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded.trim()) {
            if decoded.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&decoded);
                return key;
            } else {
                eprintln!("⚠️ Invalid key length found. Regenerating secure key...");
            }
        } else {
            eprintln!("⚠️ Corrupted key data. Regenerating secure key...");
        }
    }

    // Generate new key and save
    let mut rng = rand::rng();
    let key: [u8; 32] = rng.random();
    let encoded = base64::engine::general_purpose::STANDARD.encode(key);
    fs::write(key_path, encoded).expect("Failed to write key file");
    key
}

fn encrypt_data(data: &str) -> String {
    let key = generate_or_get_key();
    let key_array = GenericArray::from_slice(&key);
    let cipher = Aes256Gcm::new(key_array);
    let mut rng = rand::rng();
    let nonce_bytes: [u8; 12] = rng.random();
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, data.as_bytes()).unwrap();
    let mut result = nonce_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    base64::engine::general_purpose::STANDARD.encode(result)
}

fn decrypt_data(encrypted: &str) -> Result<String, String> {
    let encrypted = encrypted.trim();
    let key = generate_or_get_key();
    let key_array = GenericArray::from_slice(&key);
    let cipher = Aes256Gcm::new(key_array);

    let data = base64::engine::general_purpose::STANDARD.decode(encrypted)
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
    println!("NOTE: Fedora requires additional packages:");
    println!("sudo dnf install android-tools sox alsa-utils nmap-ncat");

    // Step 1: Check ADB
    println!("Step 1: Checking ADB installation...");
    if !check_adb_installed() {
        println!("❌ ADB not found. Please install Android SDK Platform Tools first.");
        println!("Fedora: sudo dnf install android-tools");
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
    let output = Command::new("adb")
        .args(&["connect", &format!("{}:5555", ip)])
        .output()
        .expect("Failed to run adb connect");

    if !output.status.success() {
        println!("❌ Wireless connection failed. Please retry setup.");
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("Error details: {}", stderr);
        return;
    }

    // Step 7: Save device
    save_device_ip(&ip);
    set_current_device(&format!("{}:5555", ip));

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
    let output = Command::new("adb")
        .args(&["devices", "-l"])
        .output()
        .expect("Failed to run adb devices");

    let mut devices = HashMap::new();
    let out = String::from_utf8_lossy(&output.stdout);

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
    let output = Command::new("adb")
        .args(&["-s", device_id, "shell", "ip", "addr", "show", "wlan0"])
        .output()
        .expect("Failed to get IP address");

    if !output.status.success() {
        // Try alternative method if wlan0 fails
        let output = Command::new("adb")
            .args(&["-s", device_id, "shell", "ifconfig"])
            .output()
            .expect("Failed to get IP address");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let re = Regex::new(r"inet (\d+\.\d+\.\d+\.\d+)").unwrap();

        re.captures_iter(&stdout)
            .next()
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let re = Regex::new(r"inet (\d+\.\d+\.\d+\.\d+)").unwrap();

        re.captures_iter(&stdout)
            .next()
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    }
}

fn save_device_ip(ip: &str) {
    let mut existing = HashSet::new();
    let path = get_device_file_path();

    if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        for line in content.lines() {
            existing.insert(line.trim().to_string());
        }
    }

    if !existing.contains(ip) {
        existing.insert(ip.to_string());
        let content = existing.into_iter().collect::<Vec<_>>().join("\n");
        fs::write(path, content).expect("Unable to store device IP");
    }
}

fn set_current_device(ip: &str) {
    let path = dirs::home_dir().unwrap().join(".phonectl_current");
    fs::write(path, ip).expect("Failed to write current device");
}

fn configure_unlock() {
    println!("🔐 Configure Phone Unlock");
    println!("=========================");
    println!("Enter your phone unlock method:");
    println!("1. PIN/Password");
    println!("2. Pattern");
    print!("Select option (1/2): ");
    io::stdout().flush().unwrap();

    let mut option = String::new();
    io::stdin().read_line(&mut option).unwrap();
    let option = option.trim();

    if option == "1" {
        print!("Enter your phone unlock PIN/password: ");
        io::stdout().flush().unwrap();
        let mut pin = String::new();
        io::stdin().read_line(&mut pin).unwrap();
        let pin = pin.trim();

        if pin.is_empty() {
            println!("❌ Unlock code cannot be empty.");
            return;
        }

        let encrypted = encrypt_data(&format!("PIN:{}", pin));
        fs::write(get_config_path(), encrypted).expect("Failed to write unlock config");
        println!("✅ Unlock PIN/password saved successfully (encrypted).");
    } else if option == "2" {
        println!("🔐 Enter your unlock pattern using the 3×3 grid below:");
        println!("+---+---+---+");
        println!("| 1 | 2 | 3 |");
        println!("+---+---+---+");
        println!("| 4 | 5 | 6 |");
        println!("+---+---+---+");
        println!("| 7 | 8 | 9 |");
        println!("+---+---+---+");
        println!("👉 Example: For an 'L' pattern, enter 14789");
        print!("🧩 Pattern: ");

        io::stdout().flush().unwrap();
        let mut pattern = String::new();
        io::stdin().read_line(&mut pattern).unwrap();
        let pattern = pattern.trim();

        if pattern.is_empty() {
            println!("❌ Pattern cannot be empty.");
            return;
        }

        if !pattern.chars().all(|c| c.is_ascii_digit() && c >= '1' && c <= '9') {
            println!("❌ Invalid pattern. Only numbers 1-9 allowed.");
            return;
        }

        let encrypted = encrypt_data(&format!("PATTERN:{}", pattern));
        fs::write(get_config_path(), encrypted).expect("Failed to write unlock config");
        println!("✅ Unlock pattern saved successfully (encrypted).");
    } else {
        println!("❌ Invalid option selected.");
        return;
    }

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

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("connected") {
                set_current_device(ip);
                println!("✅ Successfully connected to {}", ip);
                connected = true;
                break;
            }
        } else {
            println!("❌ Failed to connect to {}", ip);
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("Error details: {}", stderr);
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
        println!("⚠️  No unlock credentials saved. Run 'phonectl config' first.");
        return;
    }

    let encrypted = fs::read_to_string(path).unwrap();
    let credential = match decrypt_data(&encrypted) {
        Ok(p) => p,
        Err(e) => {
            println!("❌ Failed to decrypt unlock code: {}", e);
            println!("💡 Run 'phonectl config' to reconfigure.");
            return;
        }
    };

    animation_spinner("Unlocking phone");

    // Get screen dimensions for device-agnostic gestures
    let screen_info = adb_output(&["shell", "dumpsys", "window", "displays"]);
    let (width, height) = parse_screen_dimensions(&screen_info);

    // Wake up the screen
    adb(&["shell", "input", "keyevent", "KEYCODE_WAKEUP"]);
    sleep(Duration::from_millis(500));

    // Check credential type
    if credential.starts_with("PIN:") {
        try_pin_unlock(&credential[4..], width, height);
    } else if credential.starts_with("PATTERN:") {
        try_pattern_unlock(&credential[8..], width, height);
    } else {
        println!("❌ Unknown credential type. Please reconfigure.");
        return;
    }

    // Keep screen awake
    adb(&["shell", "svc", "power", "stayon", "true"]);
    println!("✅ Phone unlocked and screen will stay awake");
}

fn try_pattern_unlock(pattern: &str, width: i32, height: i32) {
    println!("🔓 Attempting pattern unlock...");

    // Pattern grid positions (3x3 grid)
    let grid_size = 3;
    let grid_width = width / grid_size;
    let grid_height = height / grid_size;

    // Calculate positions for each number
    let positions: Vec<(i32, i32)> = (1..=9)
        .map(|num| {
            let row = (num - 1) / grid_size;
            let col = (num - 1) % grid_size;
            (
                (col as i32 * grid_width) + (grid_width / 2),
                (row as i32 * grid_height) + (grid_height / 2),
            )
        })
        .collect();

    // Convert pattern to coordinates
    let mut pattern_points = Vec::new();
    for c in pattern.chars() {
        if let Some(digit) = c.to_digit(10) {
            if digit > 0 && digit <= 9 {
                pattern_points.push(positions[(digit - 1) as usize]);
            }
        }
    }

    if pattern_points.is_empty() {
        println!("❌ Invalid pattern format");
        return;
    }

    // Start from first point
    let (start_x, start_y) = pattern_points[0];

    // Generate swipe command
    let mut swipe_cmd = format!("{} {}", start_x, start_y);
    for (x, y) in pattern_points.iter().skip(1) {
        swipe_cmd.push_str(&format!(" {} {} 100", x, y));
    }

    // Execute pattern swipe
    adb(&["shell", "input", "swipe", &swipe_cmd]);
    sleep(Duration::from_millis(1000));

    println!("✅ Pattern unlock executed");
}

fn try_pin_unlock(pin: &str, width: i32, height: i32) {
    println!("🔓 Attempting PIN unlock...");

    // 1. Swipe up to show lock screen
    let start_y = (height as f32 * 0.8) as i32;
    let end_y = (height as f32 * 0.2) as i32;
    adb(&[
        "shell",
        "input",
        "swipe",
        &(width / 2).to_string(),
        &start_y.to_string(),
        &(width / 2).to_string(),
        &end_y.to_string()
    ]);
    sleep(Duration::from_millis(500));

    // 2. Enter PIN/password
    adb(&["shell", "input", "text", &pin.trim().replace(" ", "")]);
    sleep(Duration::from_millis(300));

    // 3. Press Enter
    adb(&["shell", "input", "keyevent", "KEYCODE_ENTER"]);
    sleep(Duration::from_millis(500));

    // 4. Alternative method: Try DPAD_CENTER if ENTER doesn't work
    adb(&["shell", "input", "keyevent", "KEYCODE_DPAD_CENTER"]);
    sleep(Duration::from_millis(500));
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
    print!("\r");
    io::stdout().flush().unwrap();
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

fn adb_output(args: &[&str]) -> String {
    let output = Command::new("adb")
        .args(args)
        .output()
        .expect("Failed to execute ADB command");

    String::from_utf8_lossy(&output.stdout).to_string()
}

fn parse_screen_dimensions(info: &str) -> (i32, i32) {
    // Default dimensions for common devices
    let mut width = 1080;
    let mut height = 1920;

    // Try to parse actual dimensions
    if let Some(size) = info.find("init=") {
        let rest = &info[size+5..];
        if let Some(end) = rest.find(' ') {
            let dims = &rest[..end];
            let parts: Vec<&str> = dims.split('x').collect();
            if parts.len() == 2 {
                if let (Ok(w), Ok(h)) = (parts[0].parse(), parts[1].parse()) {
                    width = w;
                    height = h;
                }
            }
        }
    }

    (width, height)
}

fn is_adb_connected() -> bool {
    let path = dirs::home_dir().unwrap().join(".phonectl_current");
    if !path.exists() {
        return false;
    }

    let current_ip = fs::read_to_string(path).unwrap_or_default();
    if current_ip.is_empty() {
        return false;
    }

    let output = Command::new("adb")
        .arg("devices")
        .output()
        .expect("Failed to run adb devices");

    let out = String::from_utf8_lossy(&output.stdout);
    out.lines().any(|l| l.contains(&current_ip) && l.contains("device"))
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

    if action == "call" {
        println!("🔄 Starting audio routing for call...");
        start_audio_routing();
    }
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

// Enhanced answer call with device-agnostic approach
fn answer_call() {
    println!("📞 Answering call...");

    // Universal method 1: Headset hook (works for most devices)
    adb(&["shell", "input", "keyevent", "KEYCODE_HEADSETHOOK"]);
    sleep(Duration::from_millis(500));

    // Universal method 2: Call button
    adb(&["shell", "input", "keyevent", "KEYCODE_CALL"]);
    sleep(Duration::from_millis(500));

    // Adaptive swipe method based on screen size
    let screen_info = adb_output(&["shell", "dumpsys", "window", "displays"]);
    let (width, height) = parse_screen_dimensions(&screen_info);

    // Calculate swipe positions (from bottom-right to center)
    let start_x = (width as f32 * 0.8) as i32;
    let start_y = (height as f32 * 0.8) as i32;
    let end_x = width / 2;
    let end_y = height / 2;

    adb(&[
        "shell",
        "input",
        "swipe",
        &start_x.to_string(),
        &start_y.to_string(),
        &end_x.to_string(),
        &end_y.to_string(),
        "1000"  // Duration in ms
    ]);

    println!("✅ Call answered");
}

// Enhanced end call with multiple methods
fn end_call() {
    println!("📞 Ending call...");

    // Try each method until one succeeds
    let methods = [
        ("KEYCODE_ENDCALL", "Standard end call"),
        ("KEYCODE_HEADSETHOOK", "Headset hook"),
        ("KEYCODE_BACK", "Back button"),
    ];

    for (keycode, description) in methods.iter() {
        println!("- Trying {} method...", description);
        let success = adb_command_success(&["shell", "input", "keyevent", keycode]);
        sleep(Duration::from_millis(500));

        if success {
            println!("✅ Call ended using {}", description);
            return;
        }
    }

    println!("⚠️ Failed to end call. Phone might already be idle.");
}

fn adb_command_success(args: &[&str]) -> bool {
    Command::new("adb")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// Audio routing functions
fn handle_audio(command: &str) {
    match command {
        "start" => start_audio_routing(),
        "stop" => stop_audio_routing(),
        "status" => check_audio_status(),
        _ => println!("Invalid audio command. Use: start, stop, status"),
    }
}

fn command_exists(command: &str) -> bool {
    Command::new("which")
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn install_sndcpy_apk() -> Result<(), String> {
    println!("📱 Installing sndcpy APK on phone...");

    // First, create the APK file in a temporary location
    let temp_dir = std::env::temp_dir();
    let apk_path = temp_dir.join("sndcpy.apk");

    // Write the embedded APK data to the temporary file
    fs::write(&apk_path, APK)
        .map_err(|e| format!("Failed to write APK file: {}", e))?;

    // Check if APK file was created successfully
    if !apk_path.exists() {
        return Err("APK file was not created successfully".to_string());
    }

    println!("📦 APK file created at: {}", apk_path.display());

    // Try to uninstall existing version first (ignore errors)
    println!("🔄 Removing existing sndcpy installation...");
    let _ = Command::new("adb")
        .args(&["uninstall", "com.rom1v.sndcpy"])
        .output(); // Don't fail if this doesn't work

    // Wait a bit for uninstall to complete
    sleep(Duration::from_millis(1000));

    // Install the APK
    println!("📲 Installing sndcpy APK...");
    let install_result = Command::new("adb")
        .args(&["install", "-r", apk_path.to_str().unwrap()])
        .output()
        .map_err(|e| format!("Failed to execute ADB install: {}", e))?;

    // Clean up the temporary APK file
    let _ = fs::remove_file(&apk_path);

    if install_result.status.success() {
        println!("✅ sndcpy APK installed successfully");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&install_result.stderr);
        let stdout = String::from_utf8_lossy(&install_result.stdout);
        Err(format!("APK installation failed:\nSTDOUT: {}\nSTDERR: {}", stdout, stderr))
    }
}

// Modified start_audio_routing function with proper APK handling
fn start_audio_routing() {
    if AUDIO_ACTIVE.load(Ordering::SeqCst) {
        println!("🔊 Audio routing is already active");
        return;
    }

    println!("🔊 Starting audio routing...");

    // Step 1: Install APK on phone if needed
    println!("📱 Checking sndcpy APK installation...");
    let apk_installed = Command::new("adb")
        .args(&["shell", "pm", "list", "packages", "com.rom1v.sndcpy"])
        .output()
        .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
        .unwrap_or(false);

    if !apk_installed {
        println!("📦 Installing sndcpy APK...");
        if let Err(e) = install_sndcpy_apk() {
            println!("❌ Failed to install sndcpy APK: {}", e);
            return;
        }
    } else {
        println!("✅ sndcpy APK already installed");
    }

    // Step 2: Set up port forwarding
    println!("🔄 Setting up port forwarding...");
    let forward = Command::new("adb")
        .args(&["forward", &format!("tcp:{}", AUDIO_FORWARD_PORT), &format!("tcp:{}", AUDIO_FORWARD_PORT)])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !forward {
        println!("❌ Failed to set up audio forwarding port");
        return;
    }

    // Step 3: Start sndcpy app on phone
    println!("🎵 Starting sndcpy app on phone...");
    let start_app = Command::new("adb")
        .args(&["shell", "am", "start", "-n", "com.rom1v.sndcpy/.MainActivity"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !start_app {
        println!("❌ Failed to start sndcpy app on phone");
        return;
    }

    // Step 4: Wait for app to initialize
    sleep(Duration::from_secs(2));

    // Step 5: Start audio capture on PC
    println!("🎧 Starting audio capture on PC...");

    // Check if we have the PC client
    if !command_exists("sndcpy") {
        println!("🔄 sndcpy PC client not found. Installing...");
        match install_sndcpy_client() {
            Ok(_) => println!("✅ sndcpy PC client installed successfully"),
            Err(e) => {
                println!("❌ Failed to install sndcpy PC client: {}", e);
                println!("💡 You can manually download from: https://github.com/rom1v/sndcpy/releases");
                return;
            }
        }
    }

    // Start the PC client
    let audio_process = Command::new("sndcpy")
        .arg(&format!("localhost:{}", AUDIO_FORWARD_PORT))
        .spawn();

    match audio_process {
        Ok(_) => {
            AUDIO_ACTIVE.store(true, Ordering::SeqCst);
            println!("🎧 Audio routing active!");
            println!("💡 Audio from phone calls will now play through your PC speakers");
        }
        Err(e) => {
            println!("❌ Failed to start audio routing: {}", e);
        }
    }
}

// Separate function to install the PC client
fn install_sndcpy_client() -> Result<(), String> {
    println!("📥 Installing sndcpy PC client...");

    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let bin_dir = home.join(".local/bin");
    fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Failed to create bin directory: {}", e))?;

    // Download the appropriate binary for the system
    let download_url = if cfg!(target_os = "linux") {
        "https://github.com/rom1v/sndcpy/releases/download/v1.1/sndcpy-v1.1-linux.zip"
    } else if cfg!(target_os = "windows") {
        "https://github.com/rom1v/sndcpy/releases/download/v1.1/sndcpy-v1.1-windows.zip"
    } else {
        return Err("Unsupported operating system".to_string());
    };

    // Download and extract
    let response = reqwest::blocking::get(download_url)
        .map_err(|e| format!("Failed to download: {}", e))?;

    let temp_dir = tempfile::tempdir()
        .map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let zip_path = temp_dir.path().join("sndcpy.zip");

    let mut file = std::fs::File::create(&zip_path)
        .map_err(|e| format!("Failed to create zip file: {}", e))?;

    let bytes = response.bytes()
        .map_err(|e| format!("Failed to read response: {}", e))?;

    std::io::Write::write_all(&mut file, &bytes)
        .map_err(|e| format!("Failed to write zip: {}", e))?;

    // Extract the binary
    let file = std::fs::File::open(&zip_path)
        .map_err(|e| format!("Failed to open zip: {}", e))?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to read zip: {}", e))?;

    // Find and extract the binary
    let binary_name = if cfg!(target_os = "windows") { "sndcpy.exe" } else { "sndcpy" };

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        if file.name().ends_with(binary_name) {
            let output_path = bin_dir.join(binary_name);
            let mut output_file = std::fs::File::create(&output_path)
                .map_err(|e| format!("Failed to create output file: {}", e))?;

            std::io::copy(&mut file, &mut output_file)
                .map_err(|e| format!("Failed to extract binary: {}", e))?;

            // Make executable on Unix systems
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&output_path)
                    .map_err(|e| format!("Failed to get permissions: {}", e))?
                    .permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&output_path, perms)
                    .map_err(|e| format!("Failed to set permissions: {}", e))?;
            }

            println!("✅ sndcpy installed to ~/.local/bin");
            return Ok(());
        }
    }

    Err("Binary not found in archive".to_string())
}

// Enhanced debug function for troubleshooting
fn debug_sndcpy_setup() {
    println!("🔍 Debug: sndcpy Setup Information");
    println!("==================================");

    // Check ADB connection
    let adb_devices = Command::new("adb")
        .args(&["devices"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_else(|_| "ADB command failed".to_string());

    println!("📱 ADB Devices:");
    println!("{}", adb_devices);

    // Check if APK is installed
    let apk_check = Command::new("adb")
        .args(&["shell", "pm", "list", "packages", "com.rom1v.sndcpy"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_else(|_| "Package check failed".to_string());

    println!("📦 APK Installation Status:");
    if apk_check.trim().is_empty() {
        println!("❌ sndcpy APK not installed");
    } else {
        println!("✅ sndcpy APK installed: {}", apk_check.trim());
    }

    // Check PC client
    let pc_client = command_exists("sndcpy");
    println!("💻 PC Client Status: {}", if pc_client { "✅ Available" } else { "❌ Not found" });

    // Check port forwarding
    let port_forward = Command::new("adb")
        .args(&["forward", "--list"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_else(|_| "Port forward check failed".to_string());

    println!("🔌 Port Forwarding:");
    if port_forward.trim().is_empty() {
        println!("❌ No port forwarding active");
    } else {
        println!("✅ Active forwards: {}", port_forward.trim());
    }

    // Check required system tools
    let tools = ["adb", "arecord", "ncat", "nc"];
    println!("🛠️ System Tools:");
    for tool in &tools {
        let available = command_exists(tool);
        println!("  • {}: {}", tool, if available { "✅" } else { "❌" });
    }
}

// Also fix the start_audio_routing function to handle the error properly


fn stop_audio_routing() {
    if !AUDIO_ACTIVE.load(Ordering::SeqCst) {
        println!("🔇 Audio routing is not active");
        return;
    }

    println!("🔇 Stopping audio routing...");

    // Kill audio processes
    let _ = Command::new("pkill")
        .arg("sndcpy")
        .status();
    let _ = Command::new("pkill")
        .arg("arecord")
        .status();
    let _ = Command::new("pkill")
        .arg("nc")
        .status();

    // Remove port forwarding
    let _ = Command::new("adb")
        .args(&["forward", "--remove", &format!("tcp:{}", AUDIO_FORWARD_PORT)])
        .status();

    AUDIO_ACTIVE.store(false, Ordering::SeqCst);
    println!("✅ Audio routing stopped");
}

fn check_audio_status() {
    println!("🎧 Audio Routing Status:");

    // Check sndcpy
    let sndcpy_running = Command::new("pgrep")
        .arg("sndcpy")
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    // Check arecord
    let arecord_running = Command::new("pgrep")
        .arg("arecord")
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    // Check netcat
    let nc_running = Command::new("pgrep")
        .arg("nc")
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    // Check port forwarding
    let port_forwarded = Command::new("adb")
        .args(&["forward", "--list"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("tcp:{}", AUDIO_FORWARD_PORT)))
        .unwrap_or(false);

    println!("  • Audio output active: {}", if sndcpy_running { "✅" } else { "❌" });
    println!("  • Microphone input active: {}", if arecord_running { "✅" } else { "❌" });
    println!("  • Netcat routing active: {}", if nc_running { "✅" } else { "❌" });
    println!("  • Port forwarding active: {}", if port_forwarded { "✅" } else { "❌" });

    let ncat_running = Command::new("pgrep")
        .arg("ncat")
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if sndcpy_running && arecord_running &&  ncat_running && port_forwarded {
        println!("🔊 Audio routing is fully operational");
    } else if !sndcpy_running && !arecord_running && ! ncat_running && !port_forwarded {
        println!("🔇 Audio routing is inactive");
    } else {
        println!("⚠️ Audio routing is partially active");
    }
}