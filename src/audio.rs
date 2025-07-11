use std::fs::{self, File};
use std::io::copy;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;
use crate::config::AUDIO_FORWARD_PORT;
use dirs;
use reqwest;
use tempfile;
use zip;

static AUDIO_ACTIVE: AtomicBool = AtomicBool::new(false);

const APK: &[u8] = include_bytes!("../assets/sndcpy.apk");

pub fn handle_audio(command: &str) {
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

    let temp_dir = std::env::temp_dir();
    let apk_path = temp_dir.join("sndcpy.apk");

    fs::write(&apk_path, APK)
        .map_err(|e| format!("Failed to write APK file: {}", e))?;

    if !apk_path.exists() {
        return Err("APK file was not created successfully".to_string());
    }

    println!("📦 APK file created at: {}", apk_path.display());

    println!("🔄 Removing existing sndcpy installation...");
    let _ = Command::new("adb")
        .args(&["uninstall", "com.rom1v.sndcpy"])
        .output();

    sleep(Duration::from_millis(1000));

    println!("📲 Installing sndcpy APK...");
    let install_result = Command::new("adb")
        .args(&["install", "-r", apk_path.to_str().unwrap()])
        .output()
        .map_err(|e| format!("Failed to execute ADB install: {}", e))?;

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

pub fn start_audio_routing() {
    if AUDIO_ACTIVE.load(Ordering::SeqCst) {
        println!("🔊 Audio routing is already active");
        return;
    }

    println!("🔊 Starting audio routing...");

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

    sleep(Duration::from_secs(2));

    println!("🎧 Starting audio capture on PC...");
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

fn install_sndcpy_client() -> Result<(), String> {
    println!("📥 Installing sndcpy PC client...");

    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let bin_dir = home.join(".local/bin");
    fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Failed to create bin directory: {}", e))?;

    let download_url = if cfg!(target_os = "linux") {
        "https://github.com/rom1v/sndcpy/releases/download/v1.1/sndcpy-v1.1-linux.zip"
    } else if cfg!(target_os = "windows") {
        "https://github.com/rom1v/sndcpy/releases/download/v1.1/sndcpy-v1.1-windows.zip"
    } else {
        return Err("Unsupported operating system".to_string());
    };

    let response = reqwest::blocking::get(download_url)
        .map_err(|e| format!("Failed to download: {}", e))?;

    let temp_dir = tempfile::tempdir()
        .map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let zip_path = temp_dir.path().join("sndcpy.zip");

    let mut file = File::create(&zip_path)
        .map_err(|e| format!("Failed to create zip file: {}", e))?;

    let bytes = response.bytes()
        .map_err(|e| format!("Failed to read response: {}", e))?;

    std::io::Write::write_all(&mut file, &bytes)
        .map_err(|e| format!("Failed to write zip: {}", e))?;

    let file = File::open(&zip_path)
        .map_err(|e| format!("Failed to open zip: {}", e))?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to read zip: {}", e))?;

    let binary_name = if cfg!(target_os = "windows") { "sndcpy.exe" } else { "sndcpy" };

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        if file.name().ends_with(binary_name) {
            let output_path = bin_dir.join(binary_name);
            let mut output_file = File::create(&output_path)
                .map_err(|e| format!("Failed to create output file: {}", e))?;

            copy(&mut file, &mut output_file)
                .map_err(|e| format!("Failed to extract binary: {}", e))?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&output_path)
                    .map_err(|e| format!("Failed to get permissions: {}", e))?
                    .permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&output_path, perms)
                    .map_err(|e| format!("Failed to set permissions: {}", e))?;
            }

            println!("✅ sndcpy installed to ~/.local/bin");
            return Ok(());
        }
    }

    Err("Binary not found in archive".to_string())
}

pub fn stop_audio_routing() {
    if !AUDIO_ACTIVE.load(Ordering::SeqCst) {
        println!("🔇 Audio routing is not active");
        return;
    }

    println!("🔇 Stopping audio routing...");

    let _ = Command::new("pkill").arg("sndcpy").status();
    let _ = Command::new("pkill").arg("arecord").status();
    let _ = Command::new("pkill").arg("nc").status();

    let _ = Command::new("adb")
        .args(&["forward", "--remove", &format!("tcp:{}", AUDIO_FORWARD_PORT)])
        .status();

    AUDIO_ACTIVE.store(false, Ordering::SeqCst);
    println!("✅ Audio routing stopped");
}

pub fn check_audio_status() {
    println!("🎧 Audio Routing Status:");

    let sndcpy_running = Command::new("pgrep")
        .arg("sndcpy")
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    let arecord_running = Command::new("pgrep")
        .arg("arecord")
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    let nc_running = Command::new("pgrep")
        .arg("nc")
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

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

    if sndcpy_running && arecord_running && ncat_running && port_forwarded {
        println!("🔊 Audio routing is fully operational");
    } else if !sndcpy_running && !arecord_running && !ncat_running && !port_forwarded {
        println!("🔇 Audio routing is inactive");
    } else {
        println!("⚠️ Audio routing is partially active");
    }
}

pub fn debug_sndcpy_setup() {
    println!("🔍 Debug: sndcpy Setup Information");
    println!("==================================");

    let adb_devices = Command::new("adb")
        .args(&["devices"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_else(|_| "ADB command failed".to_string());

    println!("📱 ADB Devices:");
    println!("{}", adb_devices);

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

    let pc_client = command_exists("sndcpy");
    println!("💻 PC Client Status: {}", if pc_client { "✅ Available" } else { "❌ Not found" });

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

    let tools = ["adb", "arecord", "ncat", "nc"];
    println!("🛠️ System Tools:");
    for tool in &tools {
        let available = command_exists(tool);
        println!("  • {}: {}", tool, if available { "✅" } else { "❌" });
    }
}