
use std::fs::{self, File};
use std::io::{self, Write, BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio, Child};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, sleep, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use regex::Regex;
use crate::adb::{adb_output, ensure_adb_connected};
use crate::config::get_camera_config_path;
use dirs;
use std::io::Read;

static CAMERA_ACTIVE: AtomicBool = AtomicBool::new(false);
static RECORDING_ACTIVE: AtomicBool = AtomicBool::new(false);

const IP_WEBCAM_APK: &[u8] = include_bytes!("../assets/ip-webcam.apk");
const DEFAULT_CAMERA_PORT: &str = "8080";
const DEFAULT_STREAM_URL_PATH: &str = "/video";
const DEFAULT_PHOTO_URL_PATH: &str = "/photo.jpg";
const DEFAULT_AUDIO_URL_PATH: &str = "/audio.wav";
const STREAM_TIMEOUT_SECS: u64 = 30;

// Global handle for the audio process
static AUDIO_PROCESS: Mutex<Option<Child>> = Mutex::new(None);

// Global handle for the video process
static VIDEO_PROCESS: Mutex<Option<Child>> = Mutex::new(None);

pub fn handle_camera(command: &str, args: &[String]) {
    match command {
        "start" => start_camera_stream(),
        "stop" => stop_camera_stream(),
        "record" => {
            let duration = args.get(0).and_then(|s| s.parse().ok()).unwrap_or(30);
            start_recording(duration);
        },
        "capture" => capture_photo(),
        "status" => check_camera_status(),
        "install" => {
            if let Err(e) = install_ip_webcam_apk() {
                println!("❌ Failed to install IP Webcam APK: {}", e);
            }
        },
        _ => {
            println!("Invalid camera command. Available commands:");
            println!("  start   - Start camera stream");
            println!("  stop    - Stop camera stream");
            println!("  record  - Record video (default 30 seconds)");
            println!("  capture - Capture photo");
            println!("  status  - Show camera status");
            println!("  install - Install IP Webcam APK");
        }
    }
}

fn get_device_ip() -> Option<String> {
    if let Some(current_device) = crate::adb::get_current_device() {
        if current_device.contains(':') {
            // TCP connection
            let mut parts = current_device.split(':');
            let ip = parts.next().unwrap_or(&current_device).to_string();
            if ip == "localhost" {
                Some("127.0.0.1".to_string())
            } else {
                Some(ip)
            }
        } else {
            // USB connection
            Some("127.0.0.1".to_string())
        }
    } else {
        None
    }
}

fn install_ip_webcam_apk() -> Result<(), String> {
    println!("📱 Installing IP Webcam APK on phone...");

    let temp_dir = std::env::temp_dir();
    let apk_path = temp_dir.join("ip_webcam.apk");

    // Write embedded APK to temporary file
    fs::write(&apk_path, IP_WEBCAM_APK)
        .map_err(|e| format!("Failed to write APK file: {}", e))?;

    if !apk_path.exists() {
        return Err("APK file was not created successfully".to_string());
    }

    println!("📦 APK file created at: {}", apk_path.display());

    // Remove existing installation
    println!("🔄 Removing existing IP Webcam installation...");
    let _ = Command::new("adb")
        .args(&["uninstall", "com.pas.webcam"])
        .output();

    sleep(Duration::from_millis(1000));

    // Install new APK
    println!("📲 Installing IP Webcam APK...");
    let install_result = Command::new("adb")
        .args(&["install", "-r", apk_path.to_str().unwrap()])
        .output()
        .map_err(|e| format!("Failed to execute ADB install: {}", e))?;

    // Clean up temporary file
    let _ = fs::remove_file(&apk_path);

    if install_result.status.success() {
        println!("✅ IP Webcam APK installed successfully");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&install_result.stderr);
        let stdout = String::from_utf8_lossy(&install_result.stdout);
        Err(format!("APK installation failed:\nSTDOUT: {}\nSTDERR: {}", stdout, stderr))
    }
}

fn check_ffmpeg_installed() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn check_ip_webcam_installed() -> bool {
    Command::new("adb")
        .args(&["shell", "pm", "list", "packages", "com.pas.webcam"])
        .output()
        .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
        .unwrap_or(false)
}

fn wait_for_stream_availability(stream_url: &str, timeout_secs: u64) -> bool {
    println!("⏳ Waiting for camera stream to become available...");
    let start_time = std::time::Instant::now();
    
    while start_time.elapsed().as_secs() < timeout_secs {
        let result = Command::new("curl")
            .args(&["-s", "--max-time", "3", "--head", stream_url])
            .output();
            
        if let Ok(output) = result {
            if output.status.success() {
                let response = String::from_utf8_lossy(&output.stdout);
                if response.contains("200 OK") {
                    println!("✅ Camera stream is now accessible!");
                    return true;
                }
            }
        }
        
        print!(".");
        io::stdout().flush().unwrap();
        sleep(Duration::from_secs(2));
    }
    
    println!("\n❌ Camera stream did not become available within {} seconds", timeout_secs);
    false
}

fn start_ip_webcam_app() -> bool {
    println!("🚀 Starting IP Webcam app on phone...");
    
    // Force stop first to ensure clean start
    let _ = Command::new("adb")
        .args(&["shell", "am", "force-stop", "com.pas.webcam"])
        .status();
    
    sleep(Duration::from_millis(1000));
    
    // Start using monkey
    let start_result = Command::new("adb")
        .args(&[
            "shell", "monkey", "-p", "com.pas.webcam",
            "-c", "android.intent.category.LAUNCHER", "1"
        ])
        .output();
    
    match start_result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() && (stdout.contains("Events injected: 1") || stderr.contains("Events injected: 1")) {
                println!("✅ IP Webcam app started successfully");
                return true;
            } else {
                println!("❌ Failed to start IP Webcam app: {}", stderr);
                println!("STDOUT: {}", stdout);
            }
        },
        Err(e) => {
            println!("❌ Failed to execute adb command: {}", e);
        }
    }
    
    false
}

fn setup_port_forwarding() -> bool {
    println!("🔌 Setting up port forwarding...");
    
    let port_forward_result = Command::new("adb")
        .args(&["forward", &format!("tcp:{}", DEFAULT_CAMERA_PORT), &format!("tcp:{}", DEFAULT_CAMERA_PORT)])
        .output();
    
    match port_forward_result {
        Ok(output) if output.status.success() => {
            println!("✅ Port forwarding set up successfully");
            true
        },
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("⚠️  Port forwarding warning: {}", stderr);
            true // Continue anyway, might still work
        },
        Err(e) => {
            println!("❌ Failed to set up port forwarding: {}", e);
            false
        }
    }
}

pub fn start_camera_stream() {
    if CAMERA_ACTIVE.load(Ordering::SeqCst) {
        println!("📹 Camera stream is already active");
        return;
    }

    println!("📹 Starting camera stream...");

    // Check dependencies
    if !check_ffmpeg_installed() {
        println!("❌ FFmpeg not found. Please install it:");
        println!("   sudo dnf install ffmpeg  # Fedora");
        println!("   sudo apt install ffmpeg  # Ubuntu/Debian");
        println!("   brew install ffmpeg      # macOS");
        return;
    }

    // Check if IP Webcam is installed
    if !check_ip_webcam_installed() {
        println!("📱 IP Webcam app not found. Installing...");
        if let Err(e) = install_ip_webcam_apk() {
            println!("❌ Failed to install IP Webcam APK: {}", e);
            return;
        }
    } else {
        println!("✅ IP Webcam APK already installed");
    }

    // Get device IP
    let device_ip = match get_device_ip() {
        Some(ip) => ip,
        None => {
            println!("❌ No active device found. Please connect to a device first.");
            return;
        }
    };

    println!("📱 Device IP: {}", device_ip);

    // Set up port forwarding for local connections
    if device_ip == "127.0.0.1" {
        if !setup_port_forwarding() {
            println!("⚠️  Port forwarding failed, but continuing...");
        }
    }

    // Start IP Webcam app
    if !start_ip_webcam_app() {
        println!("❌ Failed to start IP Webcam app");
        return;
    }

    // Wait for app to initialize
    println!("⏳ Waiting for IP Webcam to initialize...");
    sleep(Duration::from_secs(5));

    // Create stream URL
    let stream_url = format!("http://{}:{}{}", device_ip, DEFAULT_CAMERA_PORT, DEFAULT_STREAM_URL_PATH);
    println!("🎥 Camera stream URL: {}", stream_url);

    // Wait for stream to become available
    if !wait_for_stream_availability(&stream_url, STREAM_TIMEOUT_SECS) {
        println!("❌ Camera stream not accessible. Check if:");
        println!("   • Phone and PC are on same WiFi network");
        println!("   • IP Webcam app is running on phone");
        println!("   • Port {} is not blocked", DEFAULT_CAMERA_PORT);
        println!("   • Phone's firewall allows connections");
        return;
    }

    CAMERA_ACTIVE.store(true, Ordering::SeqCst);

    // Start video playback in popup window using ffplay
    let video_child = Command::new("ffplay")
        .args(&["-i", &stream_url, "-vf", "scale=640:480", "-window_title", "Camera Stream"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok();

    if let Some(child) = video_child {
        *VIDEO_PROCESS.lock().unwrap() = Some(child);
        println!("✅ Video stream opened in popup window");
    } else {
        println!("⚠️ Failed to open video stream in popup window");
    }

    // Create audio URL
    let audio_url = format!("http://{}:{}{}", device_ip, DEFAULT_CAMERA_PORT, DEFAULT_AUDIO_URL_PATH);
    println!("🔊 Audio stream URL: {}", audio_url);

    // Start audio playback
    let audio_child = Command::new("ffplay")
        .args(&["-nodisp", "-autoexit", "-i", &audio_url])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok();

    if let Some(child) = audio_child {
        *AUDIO_PROCESS.lock().unwrap() = Some(child);
    } else {
        println!("⚠️ Failed to start audio playback. Ensure audio is enabled in IP Webcam app settings.");
    }

    println!("✅ Camera stream started!");
    println!("💡 Use 'phonectl camera capture' to take photos");
    println!("💡 Use 'phonectl camera record <seconds>' to record video");
    println!("💡 Use 'phonectl camera stop' to stop the stream");
}

pub fn stop_camera_stream() {
    if !CAMERA_ACTIVE.load(Ordering::SeqCst) {
        println!("📹 Camera stream is not active");
        return;
    }

    println!("🛑 Stopping camera stream...");

    // Stop video process
    if let Some(mut process) = VIDEO_PROCESS.lock().unwrap().take() {
        let _ = process.kill();
        let _ = process.wait();
    }

    // Stop audio process
    if let Some(mut process) = AUDIO_PROCESS.lock().unwrap().take() {
        let _ = process.kill();
        let _ = process.wait();
    }

    // Kill any remaining FFmpeg processes
    let _ = Command::new("pkill").arg("-f").arg("ffmpeg").status();
    let _ = Command::new("pkill").arg("-f").arg("ffplay").status();

    // Remove port forwarding
    let _ = Command::new("adb")
        .args(&["forward", "--remove", &format!("tcp:{}", DEFAULT_CAMERA_PORT)])
        .status();

    // Stop IP Webcam app
    let _ = Command::new("adb")
        .args(&["shell", "am", "force-stop", "com.pas.webcam"])
        .status();

    CAMERA_ACTIVE.store(false, Ordering::SeqCst);
    RECORDING_ACTIVE.store(false, Ordering::SeqCst);
    
    println!("✅ Camera stream stopped");
}

pub fn capture_photo() {
    let device_ip = match get_device_ip() {
        Some(ip) => ip,
        None => {
            println!("❌ No active device found");
            return;
        }
    };

    // Ensure camera stream is running
    if !CAMERA_ACTIVE.load(Ordering::SeqCst) {
        println!("📹 Starting camera stream for photo capture...");
        start_camera_stream();
        sleep(Duration::from_secs(5));
        
        if !CAMERA_ACTIVE.load(Ordering::SeqCst) {
            println!("❌ Failed to start camera stream");
            return;
        }
    }

    let photo_url = format!("http://{}:{}{}", device_ip, DEFAULT_CAMERA_PORT, DEFAULT_PHOTO_URL_PATH);
    
    // Create photos directory
    let home = dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap());
    let photos_dir = home.join("phonectl_photos");
    if let Err(e) = fs::create_dir_all(&photos_dir) {
        println!("❌ Could not create photos directory: {}", e);
        return;
    }

    // Generate timestamp filename
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let filename = format!("capture_{}.jpg", timestamp);
    let filepath = photos_dir.join(&filename);

    println!("📸 Capturing photo from {}...", photo_url);
    
    // Capture photo using curl
    let capture_result = Command::new("curl")
        .args(&[
            "-s",
            "--max-time", "15",
            "--retry", "3",
            "-o", filepath.to_str().unwrap(),
            &photo_url
        ])
        .output();

    match capture_result {
        Ok(output) if output.status.success() => {
            if filepath.exists() && fs::metadata(&filepath).unwrap().len() > 1024 {
                println!("✅ Photo captured: {}", filepath.display());
                
                // Show image info using ffprobe if available
                if let Ok(probe_output) = Command::new("ffprobe")
                    .args(&["-v", "quiet", "-show_entries", "stream=width,height", "-of", "csv=s=x:p=0", filepath.to_str().unwrap()])
                    .output() 
                {
                    let dimensions = String::from_utf8_lossy(&probe_output.stdout);
                    if !dimensions.trim().is_empty() {
                        println!("📏 Image dimensions: {}", dimensions.trim());
                    }
                }
            } else {
                let _ = fs::remove_file(&filepath);
                println!("❌ Captured photo is empty or too small. Check camera stream connection.");
            }
        },
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("❌ Failed to capture photo: {}", stderr);
        },
        Err(e) => {
            println!("❌ Failed to execute curl command: {}", e);
        }
    }
}

pub fn start_recording(duration_secs: u32) {
    if RECORDING_ACTIVE.load(Ordering::SeqCst) {
        println!("🎥 Recording is already active");
        return;
    }

    let device_ip = match get_device_ip() {
        Some(ip) => ip,
        None => {
            println!("❌ No active device found");
            return;
        }
    };

    // Ensure camera stream is running
    if !CAMERA_ACTIVE.load(Ordering::SeqCst) {
        println!("📹 Starting camera stream for recording...");
        start_camera_stream();
        sleep(Duration::from_secs(5));
        
        if !CAMERA_ACTIVE.load(Ordering::SeqCst) {
            println!("❌ Failed to start camera stream");
            return;
        }
    }

    let stream_url = format!("http://{}:{}{}", device_ip, DEFAULT_CAMERA_PORT, DEFAULT_STREAM_URL_PATH);
    let audio_url = format!("http://{}:{}{}", device_ip, DEFAULT_CAMERA_PORT, DEFAULT_AUDIO_URL_PATH);
    
    // Create recordings directory
    let home = dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap());
    let recordings_dir = home.join("phonectl_recordings");
    if let Err(e) = fs::create_dir_all(&recordings_dir) {
        println!("❌ Could not create recordings directory: {}", e);
        return;
    }

    // Generate timestamp filename
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let filename = format!("recording_{}.mp4", timestamp);
    let filepath = recordings_dir.join(&filename);

    println!("🎥 Starting recording for {} seconds...", duration_secs);
    println!("📹 Recording from: {}", stream_url);
    println!("🔊 Audio from: {}", audio_url);
    RECORDING_ACTIVE.store(true, Ordering::SeqCst);

    let filepath_clone = filepath.clone();
    let stream_url_clone = stream_url.clone();
    let audio_url_clone = audio_url.clone();
    let duration_str = duration_secs.to_string();

    // Start recording with FFmpeg in a separate thread
    let recording_handle = thread::spawn(move || {
        let record_result = Command::new("ffmpeg")
            .args(&[
                "-i", &stream_url_clone,
                "-i", &audio_url_clone,
                "-t", &duration_str,
                "-c:v", "libx264",
                "-preset", "fast",
                "-crf", "23",
                "-r", "15",
                "-c:a", "aac",
                "-map", "0:v",
                "-map", "1:a",
                "-y", // Overwrite output file
                filepath_clone.to_str().unwrap()
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output();

        RECORDING_ACTIVE.store(false, Ordering::SeqCst);

        match record_result {
            Ok(output) if output.status.success() => {
                if filepath_clone.exists() && fs::metadata(&filepath_clone).unwrap().len() > 10240 {
                    println!("\n✅ Recording saved: {}", filepath_clone.display());
                    
                    // Show video info
                    if let Ok(probe_output) = Command::new("ffprobe")
                        .args(&["-v", "quiet", "-show_entries", "format=duration", "-of", "csv=p=0", filepath_clone.to_str().unwrap()])
                        .output() 
                    {
                        let duration_str = String::from_utf8_lossy(&probe_output.stdout);
                        if let Ok(duration) = duration_str.trim().parse::<f64>() {
                            println!("⏱️  Duration: {:.1} seconds", duration);
                        }
                    }
                } else {
                    println!("\n❌ Recording failed or file is too small");
                    let _ = fs::remove_file(&filepath_clone);
                }
            },
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("\n❌ Recording failed: {}", stderr);
                let _ = fs::remove_file(&filepath_clone);
            },
            Err(e) => {
                println!("\n❌ Failed to start recording: {}", e);
            }
        }
    });

    // Show progress in main thread
    for i in 1..=duration_secs {
        if !RECORDING_ACTIVE.load(Ordering::SeqCst) {
            break;
        }
        print!("\r🔴 Recording... {}s/{} s", i, duration_secs);
        io::stdout().flush().unwrap();
        sleep(Duration::from_secs(1));
    }

    // Wait for recording to complete
    let _ = recording_handle.join();
    
    if RECORDING_ACTIVE.load(Ordering::SeqCst) {
        println!("\r✅ Recording complete!            ");
        RECORDING_ACTIVE.store(false, Ordering::SeqCst);
    }
}

pub fn check_camera_status() {
    println!("📹 Camera Status:");
    
    let ip_webcam_installed = check_ip_webcam_installed();
    let ffmpeg_installed = check_ffmpeg_installed();
    let camera_active = CAMERA_ACTIVE.load(Ordering::SeqCst);
    let recording_active = RECORDING_ACTIVE.load(Ordering::SeqCst);
    
    println!("  • IP Webcam APK installed: {}", if ip_webcam_installed { "✅" } else { "❌" });
    println!("  • FFmpeg available: {}", if ffmpeg_installed { "✅" } else { "❌" });
    println!("  • Camera stream active: {}", if camera_active { "✅" } else { "❌" });
    println!("  • Recording active: {}", if recording_active { "✅" } else { "❌" });
    
    // Check ADB connection
    match crate::adb::get_current_device() {
        Some(device) => println!("  • ADB device: ✅ {}", device),
        None => println!("  • ADB device: ❌ No active device"),
    }
    
    if let Some(device_ip) = get_device_ip() {
        let display_ip = if device_ip == "127.0.0.1" { "127.0.0.1 (USB/Local)" } else { device_ip.as_str() };
        println!("  • Device IP: {}", display_ip);
        let stream_url = format!("http://{}:{}{}", device_ip, DEFAULT_CAMERA_PORT, DEFAULT_STREAM_URL_PATH);
        println!("  • Stream URL: {}", stream_url);
        
        // Test stream accessibility
        let accessible = Command::new("curl")
            .args(&["-s", "--max-time", "3", "--head", &stream_url])
            .output()
            .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("200 OK"))
            .unwrap_or(false);
        println!("  • Stream accessible: {}", if accessible { "✅" } else { "❌" });
    } else {
        println!("  • Device IP: ❌ No active device");
    }

    // Check if IP Webcam app is running
    if ip_webcam_installed {
        let app_running = Command::new("adb")
            .args(&["shell", "pgrep", "-f", "com.pas.webcam"])
            .output()
            .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(false);
        println!("  • IP Webcam app running: {}", if app_running { "✅" } else { "❌" });
    }

    // Show directories
    let home = dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap());
    println!("  • Photos directory: {}/phonectl_photos", home.display());
    println!("  • Recordings directory: {}/phonectl_recordings", home.display());
}