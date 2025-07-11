use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;
use crate::config::get_device_file_path;
use regex::Regex;

pub fn check_adb_installed() -> bool {
    Command::new("adb")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn adb(args: &[&str]) {
    let status = Command::new("adb").args(args).status().unwrap_or_else(|_| {
        eprintln!("❌ Failed to execute ADB command: {:?}", args);
        std::process::exit(1);
    });

    if !status.success() {
        eprintln!("❌ ADB command failed: {:?}", args);
    }
}

pub fn adb_output(args: &[&str]) -> String {
    let output = Command::new("adb")
        .args(args)
        .output()
        .expect("Failed to execute ADB command");
    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn get_usb_devices() -> HashMap<String, String> {
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

pub fn get_device_ip(device_id: &str) -> String {
    let output = Command::new("adb")
        .args(&["-s", device_id, "shell", "ip", "addr", "show", "wlan0"])
        .output()
        .expect("Failed to get IP address");

    if !output.status.success() {
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

pub fn save_device_ip(ip: &str) {
    let mut existing = std::collections::HashSet::new();
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

pub fn set_current_device(ip: &str) {
    let path = dirs::home_dir().unwrap().join(".phonectl_current");
    fs::write(path, ip).expect("Failed to write current device");
}

pub fn is_adb_connected() -> bool {
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

pub fn ensure_adb_connected<F: FnOnce()>(func: F) {
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

pub fn setup_wizard() {
    println!("🚀 Welcome to ADB Wireless Setup Wizard!");
    println!("========================================");
    println!("");
    println!("NOTE: Fedora requires additional packages:");
    println!("sudo dnf install android-tools sox alsa-utils nmap-ncat");

    println!("Step 1: Checking ADB installation...");
    if !check_adb_installed() {
        println!("❌ ADB not found. Please install Android SDK Platform Tools first.");
        println!("Fedora: sudo dnf install android-tools");
        return;
    }
    println!("✅ ADB is installed and ready");

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

    println!("Step 5: Detecting device IP address...");
    let ip = get_device_ip(&device_id);
    if ip.is_empty() {
        println!("❌ Could not detect device IP. Please ensure:");
        println!("  • Phone is connected to Wi-Fi");
        println!("  • Phone and computer are on the same network");
        return;
    }
    println!("✅ Device IP detected: {}", ip);

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

    save_device_ip(&ip);
    set_current_device(&format!("{}:5555", ip));

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

pub fn reconnect_saved_device() {
    println!("🔄 Reconnecting to saved devices...");
    let path = get_device_file_path();
    if !path.exists() {
        println!("❌ No saved devices found.");
        println!("💡 Run 'phonectl setup' to configure a new device.");
        return;
    }

    let content = fs::read_to_string(path).unwrap_or_default();
    let ips: std::collections::HashSet<String> = content.lines().map(|s| s.trim().to_string()).collect();

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

pub fn show_about() {
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