use std::collections::{HashMap};
use std::fs::{self};
use std::io::{self, Read, Write};

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
        eprintln!("\x1b[31m❌ Failed to execute ADB command: {:?}", args);
        std::process::exit(1);
    });

    if !status.success() {
        eprintln!("\x1b[31m❌ ADB command failed: {:?}", args);
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

pub fn save_device_ip(ip: &str, name: Option<&str>) {
    let mut devices = load_devices();
    let name = name.unwrap_or(&format!("Device_{}", devices.len() + 1)).to_string();
    devices.insert(ip.to_string(), name);
    save_devices(&devices);
}

pub fn load_devices() -> HashMap<String, String> {
    let path = get_device_file_path();
    let mut devices = HashMap::new();
    if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        for line in content.lines() {
            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() == 2 {
                devices.insert(parts[0].to_string(), parts[1].to_string());
            }
        }
    }
    devices
}

pub fn save_devices(devices: &HashMap<String, String>) {
    let path = get_device_file_path();
    let content = devices
        .iter()
        .map(|(ip, name)| format!("{}\t{}", ip, name))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(path, content).expect("Unable to store device IPs");
}

pub fn set_current_device(ip: &str) {
    let path = dirs::home_dir().unwrap().join(".phonectl_current");
    fs::write(path, ip).expect("Failed to write current device");
}

pub fn get_current_device() -> Option<String> {
    let path = dirs::home_dir().unwrap().join(".phonectl_current");
    if path.exists() {
        let ip = fs::read_to_string(path).unwrap_or_default();
        if !ip.is_empty() {
            return Some(ip);
        }
    }
    None
}

pub fn is_adb_connected() -> bool {
    let current_ip = get_current_device();
    if current_ip.is_none() {
        return false;
    }
    let current_ip = current_ip.unwrap();

    let output = Command::new("adb")
        .arg("devices")
        .output()
        .expect("Failed to run adb devices");

    let out = String::from_utf8_lossy(&output.stdout);
    out.lines().any(|l| l.contains(&current_ip) && l.contains("device"))
}

pub fn ensure_adb_connected<F: FnOnce()>(func: F) {
    if !is_adb_connected() {
        println!("\x1b[34m🔌 ADB not connected. Attempting to reconnect...\x1b[0m");
        reconnect_saved_device();
    }

    if is_adb_connected() {
        func();
    } else {
        println!("\x1b[31m❌ ADB connection failed. Please run 'phonectl reconnect' or 'phonectl setup'.\x1b[0m");
    }
}

fn detect_distribution() -> Option<(String, String, Vec<&'static str>)> {
    let os_release = fs::read_to_string("/etc/os-release").ok()?;
    let lines: Vec<&str> = os_release.lines().collect();
    let id_line = lines.iter().find(|l| l.starts_with("ID="))?;
    let distro_id = id_line.strip_prefix("ID=").unwrap_or("unknown").trim_matches('"');

    match distro_id {
        "fedora" => Some((
            "dnf".to_string(),
            "sudo dnf install -y".to_string(),
            vec!["android-tools", "sox", "alsa-utils", "nmap-ncat", "procps"],
        )),
        "ubuntu" | "debian" => Some((
            "apt".to_string(),
            "sudo apt update && sudo apt install -y".to_string(),
            vec!["adb", "sox", "alsa-utils", "ncat", "procps"],
        )),
        "arch" | "manjaro" => Some((
            "pacman".to_string(),
            "sudo pacman -S --noconfirm".to_string(),
            vec!["android-tools", "sox", "alsa-utils", "nmap", "procps-ng"],
        )),
        "opensuse" | "opensuse-tumbleweed" => Some((
            "zypper".to_string(),
            "sudo zypper install -y".to_string(),
            vec!["android-tools", "sox", "alsa-utils", "nmap", "procps"],
        )),
        "alpine" => Some((
            "apk".to_string(),
            "sudo apk add".to_string(),
            vec!["android-tools", "sox", "alsa-utils", "nmap", "procps-ng"],
        )),
        "void" => Some((
            "xbps-install".to_string(),
            "sudo xbps-install -S".to_string(),
            vec!["android-tools", "sox", "alsa-utils", "nmap", "procps-ng"],
        )),
        _ => None,
    }
}

fn check_tool_installed(tool: &str) -> bool {
    Command::new("which")
        .arg(tool)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn get_install_mode() -> String {
    let path = dirs::home_dir().unwrap().join(".phonectl_config");
    if path.exists() {
        fs::read_to_string(&path)
            .unwrap_or_else(|_| "manual".to_string())
            .trim()
            .to_string()
    } else {
        "manual".to_string()
    }
}

fn set_install_mode(mode: &str) {
    let path = dirs::home_dir().unwrap().join(".phonectl_config");
    fs::write(path, mode).expect("Failed to write install mode");
}

pub fn config_install_mode(mode: &str) {
    let mode = mode.to_lowercase();
    if mode == "auto" || mode == "manual" {
        set_install_mode(&mode);
        println!("\x1b[32m✅ Installation mode set to: {}\x1b[0m", mode);
    } else {
        println!("\x1b[31m❌ Invalid mode. Use 'auto' or 'manual'.\x1b[0m");
    }
}

pub fn install_packages() -> bool {
    let required_tools = vec!["adb", "sox", "arecord", "ncat", "pgrep"];
    let missing_tools: Vec<&str> = required_tools
        .into_iter()
        .filter(|tool| !check_tool_installed(tool))
        .collect();

    if missing_tools.is_empty() {
        println!("\x1b[32m✅ All required tools are installed: adb, sox, arecord, ncat, pgrep\x1b[0m");
        return true;
    }

    println!("\x1b[33m⚠️ Missing required tools: {}\x1b[0m", missing_tools.join(", "));
    if let Some((pkg_manager, install_cmd, packages)) = detect_distribution() {
        println!("\x1b[34m📦 Detected distribution with package manager: {}\x1b[0m", pkg_manager);
        println!("\x1b[34m📦 Suggested command: {} {}\x1b[0m", install_cmd, packages.join(" "));

        let install_mode = get_install_mode();
        if install_mode == "auto" {
            println!("\x1b[34m📦 Automatic installation enabled. Installing packages...\x1b[0m");
            animation_spinner("Installing dependencies");
            let status = Command::new("sh")
                .arg("-c")
                .arg(&format!("{} {}", install_cmd, packages.join(" ")))
                .status()
                .map_err(|e| {
                    println!("\x1b[31m❌ Failed to execute installation command: {}\x1b[0m", e);
                });

            if let Ok(status) = status {
                if status.success() {
                    println!("\x1b[32m✅ Packages installed successfully.\x1b[0m");
                    if pkg_manager == "apk" {
                        println!("\x1b[33m⚠️ Note: You are using a musl-based system (Alpine). The sndcpy binary may not work due to glibc dependency.\x1b[0m");
                        println!("\x1b[33m💡 Please compile sndcpy from source: https://github.com/rom1v/sndcpy\x1b[0m");
                    }
                    return true;
                } else {
                    println!("\x1b[31m❌ Package installation failed. Please install manually: {}\x1b[0m", packages.join(" "));
                    return false;
                }
            }
        } else {
            // Manual mode: prompt with animation
            let spinner = ["\x1b[31m⠋\x1b[0m", "\x1b[32m⠙\x1b[0m", "\x1b[33m⠹\x1b[0m", "\x1b[34m⠸\x1b[0m", "\x1b[35m⠼\x1b[0m", "\x1b[36m⠴\x1b[0m", "\x1b[31m⠦\x1b[0m", "\x1b[32m⠧\x1b[0m", "\x1b[33m⠇\x1b[0m", "\x1b[34m⠏\x1b[0m"];
            print!("\x1b[33m📥 Would you like to install the missing packages? (y/n): \x1b[0m");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            let start_time = std::time::Instant::now();
            let mut i = 0;

            while start_time.elapsed() < Duration::from_secs(30) {
                print!("\r{} \x1b[33m📥 Would you like to install the missing packages? (y/n): \x1b[0m", spinner[i % spinner.len()]);
                io::stdout().flush().unwrap();
                i += 1;

                let mut buffer = [0; 1];
                if let Ok(n) = io::stdin().read(&mut buffer) {
                    if n > 0 {
                        input.push(buffer[0] as char);
                        if input.contains('\n') || input.contains('\r') {
                            break;
                        }
                    }
                }
                sleep(Duration::from_millis(100));
            }
            print!("\r\x1b[K"); // Clear the line
            io::stdout().flush().unwrap();

            let response = input.trim().to_lowercase();
            if response == "y" || response == "yes" {
                animation_spinner("Installing dependencies");
                let status = Command::new("sh")
                    .arg("-c")
                    .arg(&format!("{} {}", install_cmd, packages.join(" ")))
                    .status()
                    .map_err(|e| {
                        println!("\x1b[31m❌ Failed to execute installation command: {}\x1b[0m", e);
                    });

                if let Ok(status) = status {
                    if status.success() {
                        println!("\x1b[32m✅ Packages installed successfully.\x1b[0m");
                        if pkg_manager == "apk" {
                            println!("\x1b[33m⚠️ Note: You are using a musl-based system (Alpine). The sndcpy binary may not work due to glibc dependency.\x1b[0m");
                            println!("\x1b[33m💡 Please compile sndcpy from source: https://github.com/rom1v/sndcpy\x1b[0m");
                        }
                        return true;
                    } else {
                        println!("\x1b[31m❌ Package installation failed. Please install manually: {}\x1b[0m", packages.join(" "));
                    }
                }
            } else {
                println!("\x1b[31m❌ Skipping package installation. Some features may not work.\x1b[0m");
                println!("\x1b[33m💡 Manual installation command: {} {}\x1b[0m", install_cmd, packages.join(" "));
                println!("\x1b[33m💡 Run 'phonectl config-install auto' to enable automatic installation.\x1b[0m");
            }
        }
    } else {
        println!("\x1b[31m❌ Unknown Linux distribution. Please install the following packages manually:\x1b[0m");
        println!("\x1b[33m  • adb (android-tools)\x1b[0m");
        println!("\x1b[33m  • sox\x1b[0m");
        println!("\x1b[33m  • alsa-utils (arecord)\x1b[0m");
        println!("\x1b[33m  • ncat (nmap or netcat)\x1b[0m");
        println!("\x1b[33m  • pgrep (procps or procps-ng)\x1b[0m");
        println!("\x1b[33m💡 Run 'phonectl config-install auto' to enable automatic installation.\x1b[0m");
    }

    false
}

pub fn switch_device() {
    let devices = load_devices();
    if devices.is_empty() {
        println!("\x1b[31m❌ No saved devices found. Run 'phonectl setup' to add a device.\x1b[0m");
        return;
    }

    println!("\x1b[34m📱 Available devices:\x1b[0m");
    let device_list: Vec<(&String, &String)> = devices.iter().collect();
    for (i, (ip, name)) in device_list.iter().enumerate() {
        println!("\x1b[34m{}. {} ({})\x1b[0m", i + 1, name, ip);
    }

    print!("\x1b[34mSelect device number: \x1b[0m");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let idx: usize = input.trim().parse().unwrap_or(0);

    if idx == 0 || idx > device_list.len() {
        println!("\x1b[31m❌ Invalid selection.\x1b[0m");
        return;
    }

    let (ip, name) = device_list[idx - 1];
    println!("\x1b[34m🔄 Connecting to {} ({})...\x1b[0m", name, ip);
    animation_spinner("Establishing connection");
    let output = Command::new("adb")
        .args(&["connect", ip])
        .output()
        .expect("Failed to connect");

    if output.status.success() && String::from_utf8_lossy(&output.stdout).contains("connected") {
        set_current_device(ip);
        println!("\x1b[32m✅ Switched to device: {} ({})\x1b[0m", name, ip);
    } else {
        println!("\x1b[31m❌ Failed to connect to {} ({}).\x1b[0m", name, ip);
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("\x1b[31mError details: {}\x1b[0m", stderr);
    }
}

pub fn rename_device(ip: &str, new_name: &str) {
    let mut devices = load_devices();
    if !devices.contains_key(ip) {
        println!("\x1b[31m❌ Device {} not found.\x1b[0m", ip);
        return;
    }

    devices.insert(ip.to_string(), new_name.to_string());
    save_devices(&devices);
    println!("\x1b[32m✅ Device {} renamed to {}\x1b[0m", ip, new_name);

    if Some(ip.to_string()) == get_current_device() {
        set_current_device(ip); // Update current device to ensure consistency
    }
}

pub fn setup_wizard() {
    println!("\x1b[34m🚀 Welcome to ADB Wireless Setup Wizard!\x1b[0m");
    println!("\x1b[34m========================================\x1b[0m");

    println!("\x1b[34mStep 1: Checking system dependencies...\x1b[0m");
    if !install_packages() {
        println!("\x1b[31m❌ Some dependencies are missing. Setup may fail.\x1b[0m");
        println!("\x1b[33m💡 You can retry with 'phonectl setup' after installing dependencies.\x1b[0m");
        return;
    }

    println!("\x1b[32m✅ All dependencies are installed.\x1b[0m");

    println!("\x1b[34mStep 2: Checking ADB installation...\x1b[0m");
    if !check_adb_installed() {
        println!("\x1b[31m❌ ADB not found. Please ensure android-tools is installed.\x1b[0m");
        return;
    }
    println!("\x1b[32m✅ ADB is installed and ready\x1b[0m");

    println!("\x1b[34m\nStep 3: Connect your phone via 🔌 USB cable\x1b[0m");
    println!("\x1b[34m📱 Please ensure:\x1b[0m");
    println!("\x1b[34m  • USB cable is connected to your phone and computer\x1b[0m");
    println!("\x1b[34m  • USB Debugging is enabled in Developer Options\x1b[0m");
    println!("\x1b[34m  • Phone is unlocked and you've allowed USB debugging\x1b[0m");
    println!("\x1b[34m\nPress Enter when phone is connected via USB...\x1b[0m");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    println!("\x1b[34mStep 4: Checking USB connection...\x1b[0m");
    let devices = get_usb_devices();
    if devices.is_empty() {
        println!("\x1b[31m❌ Phone not detected via USB. Please check:\x1b[0m");
        println!("\x1b[31m  • USB cable is properly connected\x1b[0m");
        println!("\x1b[31m  • USB Debugging is enabled\x1b[0m");
        println!("\x1b[31m  • You've authorized this computer on your phone\x1b[0m");
        return;
    }

    let device_id = if devices.len() == 1 {
        devices.keys().next().unwrap().clone()
    } else {
        println!("\x1b[33mMultiple devices detected:\x1b[0m");
        for (i, (id, model)) in devices.iter().enumerate() {
            println!("\x1b[33m{}. {} ({})\x1b[0m", i + 1, model, id);
        }
        print!("\x1b[34mSelect the device number to set up: \x1b[0m");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let idx: usize = input.trim().parse().unwrap_or(0);
        if idx == 0 || idx > devices.len() {
            println!("\x1b[33m⚠️ Invalid selection, using first device.\x1b[0m");
            devices.keys().next().unwrap().clone()
        } else {
            devices.keys().nth(idx - 1).unwrap().clone()
        }
    };

    println!("\x1b[32m✅ Phone detected via USB: {}\x1b[0m", devices[&device_id]);

    println!("\x1b[34m\nStep 5: Enter a name for this device (e.g., MyPhone, or press Enter for default):\x1b[0m");
    print!("\x1b[34mDevice name: \x1b[0m");
    io::stdout().flush().unwrap();
    let mut device_name = String::new();
    io::stdin().read_line(&mut device_name).unwrap();
    let device_name = device_name.trim();
    let device_name = if device_name.is_empty() { None } else { Some(device_name) };

    println!("\x1b[34m\nStep 6: Enabling ADB over Wi-Fi...\x1b[0m");
    animation_spinner("Configuring ADB TCP/IP mode");
    let status = Command::new("adb")
        .args(&["-s", &device_id, "tcpip", "5555"])
        .status()
        .expect("Failed to run adb tcpip");

    if !status.success() {
        println!("\x1b[31m❌ Failed to enable ADB over Wi-Fi. Please check your device connection.\x1b[0m");
        return;
    }
    println!("\x1b[32m✅ ADB TCP/IP mode enabled\x1b[0m");
    sleep(Duration::from_secs(2));

    println!("\x1b[34mStep 7: Detecting device IP address...\x1b[0m");
    let ip = get_device_ip(&device_id);
    if ip.is_empty() {
        println!("\x1b[31m❌ Could not detect device IP. Please ensure:\x1b[0m");
        println!("\x1b[31m  • Phone is connected to Wi-Fi\x1b[0m");
        println!("\x1b[31m  • Phone and computer are on the same network\x1b[0m");
        return;
    }
    println!("\x1b[32m✅ Device IP detected: {}\x1b[0m", ip);

    println!("\x1b[34m\nStep 8: Connecting wirelessly...\x1b[0m");
    animation_spinner("Establishing wireless connection");
    let output = Command::new("adb")
        .args(&["connect", &format!("{}:5555", ip)])
        .output()
        .expect("Failed to run adb connect");

    if !output.status.success() {
        println!("\x1b[31m❌ Wireless connection failed. Please retry setup.\x1b[0m");
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("\x1b[31mError details: {}\x1b[0m", stderr);
        return;
    }

    save_device_ip(&ip, device_name);
    set_current_device(&format!("{}:5555", ip));

    println!("\x1b[34mStep 9: Testing wireless connection...\x1b[0m");
    if !is_adb_connected() {
        println!("\x1b[31m❌ Wireless connection test failed.\x1b[0m");
        return;
    }

    println!("\x1b[32m✅ Wireless connection established successfully!\x1b[0m");
    println!("\x1b[34m\n🎉 Setup Complete!\x1b[0m");
    println!("\x1b[34m=================\x1b[0m");
    println!("\x1b[34mYou can now disconnect the USB cable.\x1b[0m");
    println!("\x1b[34mYour device will automatically reconnect when both devices are on.\x1b[0m");
    println!("\x1b[34m\nNext steps:\x1b[0m");
    println!("\x1b[34m  • Run 'phonectl config' to set up phone unlock\x1b[0m");
    println!("\x1b[34m  • Run 'phonectl list' to see your contacts\x1b[0m");
    println!("\x1b[34m  • Run 'phonectl switch-device' to change active device\x1b[0m");
    println!("\x1b[34m  • Run 'phonectl rename-device <ip> <name>' to rename a device\x1b[0m");
    println!("\x1b[34m  • Run 'phonectl config-install [auto|manual]' to change installation mode\x1b[0m");
}

pub fn reconnect_saved_device() {
    println!("\x1b[34m🔄 Reconnecting to saved devices...\x1b[0m");
    let devices = load_devices();
    if devices.is_empty() {
        println!("\x1b[31m❌ No saved devices found.\x1b[0m");
        println!("\x1b[33m💡 Run 'phonectl setup' to configure a new device.\x1b[0m");
        return;
    }

    println!("\x1b[34mFound {} saved device(s):\x1b[0m", devices.len());
    for (ip, name) in &devices {
        println!("\x1b[34m  • {} ({})\x1b[0m", name, ip);
    }
    println!();

    let mut connected = false;
    for (ip, name) in &devices {
        println!("\x1b[34m🔄 Connecting to {} ({})...\x1b[0m", name, ip);
        animation_spinner("Establishing connection");
        let output = Command::new("adb")
            .args(&["connect", ip])
            .output()
            .expect("Failed to connect");

        sleep(Duration::from_millis(500));

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("connected") {
                set_current_device(ip);
                println!("\x1b[32m✅ Successfully connected to {} ({})\x1b[0m", name, ip);
                connected = true;
                break;
            }
        } else {
            println!("\x1b[31m❌ Failed to connect to {} ({})\x1b[0m", name, ip);
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("\x1b[31mError details: {}\x1b[0m", stderr);
        }
    }

    if connected {
        println!("\x1b[32m🎉 Device reconnected successfully!\x1b[0m");
    } else {
        println!("\x1b[31m❌ Could not reconnect to any saved device.\x1b[0m");
        println!("\x1b[33m💡 Make sure your phone is on the same Wi-Fi network and try again.\x1b[0m");
    }
}

pub fn show_about() {
    println!("\x1b[34m📱 Wireless ADB Phone Control CLI\x1b[0m");
    println!("\x1b[34m==========================================\x1b[0m");
    println!("\x1b[34mVersion: 1.5\x1b[0m");
    println!("\x1b[34mDeveloper: Sanjai Shaarugesh\x1b[0m");
    println!("\x1b[34mGitHub: https://github.com/Sanjai-Shaarugesh/phonectl\x1b[0m");
    println!("\x1b[34mDescription: Full-featured Rust CLI for wireless ADB control\x1b[0m");
    println!("\x1b[34mOptimized for Linux\x1b[0m");
    println!("\x1b[34mFeatures:\x1b[0m");
    println!("\x1b[34m  • Encrypted credential storage\x1b[0m");
    println!("\x1b[34m  • Pattern unlock support\x1b[0m");
    println!("\x1b[34m  • Automatic device reconnection\x1b[0m");
    println!("\x1b[34m  • Contact management\x1b[0m");
    println!("\x1b[34m  • Call functionality\x1b[0m");
    println!("\x1b[34m  • Phone unlock automation\x1b[0m");
    println!("\x1b[34m  • Full-duplex call audio routing\x1b[0m");
    println!("\x1b[34m  • Multi-device management\x1b[0m");
    println!("\x1b[34m  • Auto/manual dependency installation\x1b[0m");
    println!("\x1b[34m\nLicense: MIT\x1b[0m");
    println!("\x1b[34mBuilt with ❤️ in Rust\x1b[0m");
}

fn animation_spinner(label: &str) {
    let spinner = ["\x1b[31m⠋\x1b[0m", "\x1b[32m⠙\x1b[0m", "\x1b[33m⠹\x1b[0m", "\x1b[34m⠸\x1b[0m", "\x1b[35m⠼\x1b[0m", "\x1b[36m⠴\x1b[0m", "\x1b[31m⠦\x1b[0m", "\x1b[32m⠧\x1b[0m", "\x1b[33m⠇\x1b[0m", "\x1b[34m⠏\x1b[0m"];
    for i in 0..30 {
        print!("\r{} {}...", spinner[i % spinner.len()], label);
        io::stdout().flush().unwrap();
        sleep(Duration::from_millis(100));
    }
    print!("\r\x1b[K"); // Clear the line
    io::stdout().flush().unwrap();
}