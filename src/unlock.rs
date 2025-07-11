use std::fs;
use std::io::{self, Write};
use std::thread::sleep;
use std::time::Duration;
use crate::adb::adb;
use crate::config::get_config_path;
use crate::crypto::decrypt_data;

pub fn configure_unlock() {
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

        let encrypted = crate::crypto::encrypt_data(&format!("PIN:{}", pin));
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

        let encrypted = crate::crypto::encrypt_data(&format!("PATTERN:{}", pattern));
        fs::write(get_config_path(), encrypted).expect("Failed to write unlock config");
        println!("✅ Unlock pattern saved successfully (encrypted).");
    } else {
        println!("❌ Invalid option selected.");
        return;
    }

    println!("🔒 Your credentials are stored encrypted on disk.");
}

pub fn unlock_phone() {
    println!("🔓 Unlocking phone...");
    let path = get_config_path();
    if !path.exists() {
        println!("⚠️ No unlock credentials saved. Run 'phonectl config' first.");
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

    let screen_info = crate::adb::adb_output(&["shell", "dumpsys", "window", "displays"]);
    let (width, height) = parse_screen_dimensions(&screen_info);

    adb(&["shell", "input", "keyevent", "KEYCODE_WAKEUP"]);
    sleep(Duration::from_millis(500));

    if credential.starts_with("PIN:") {
        try_pin_unlock(&credential[4..], width, height);
    } else if credential.starts_with("PATTERN:") {
        try_pattern_unlock(&credential[8..], width, height);
    } else {
        println!("❌ Unknown credential type. Please reconfigure.");
        return;
    }

    adb(&["shell", "svc", "power", "stayon", "true"]);
    println!("✅ Phone unlocked and screen will stay awake");
}

pub fn wake_phone() {
    println!("☀️ Waking up phone...");
    adb(&["shell", "input", "keyevent", "KEYCODE_WAKEUP"]);
    adb(&["shell", "svc", "power", "stayon", "true"]);
    println!("✅ Phone screen is now awake");
}

pub fn parse_screen_dimensions(info: &str) -> (i32, i32) {
    let mut width = 1080;
    let mut height = 1920;

    if let Some(size) = info.find("init=") {
        let rest = &info[size + 5..];
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

fn try_pattern_unlock(pattern: &str, width: i32, height: i32) {
    println!("🔓 Attempting pattern unlock...");

    let grid_size = 3;
    let grid_width = width / grid_size;
    let grid_height = height / grid_size;

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

    let (start_x, start_y) = pattern_points[0];
    let mut swipe_cmd = format!("{} {}", start_x, start_y);
    for (x, y) in pattern_points.iter().skip(1) {
        swipe_cmd.push_str(&format!(" {} {} 100", x, y));
    }

    adb(&["shell", "input", "swipe", &swipe_cmd]);
    sleep(Duration::from_millis(1000));

    println!("✅ Pattern unlock executed");
}

fn try_pin_unlock(pin: &str, width: i32, height: i32) {
    println!("🔓 Attempting PIN unlock...");

    let start_y = (height as f32 * 0.8) as i32;
    let end_y = (height as f32 * 0.2) as i32;
    adb(&[
        "shell",
        "input",
        "swipe",
        &(width / 2).to_string(),
        &start_y.to_string(),
        &(width / 2).to_string(),
        &end_y.to_string(),
    ]);
    sleep(Duration::from_millis(500));

    adb(&["shell", "input", "text", &pin.trim().replace(" ", "")]);
    sleep(Duration::from_millis(300));

    adb(&["shell", "input", "keyevent", "KEYCODE_ENTER"]);
    sleep(Duration::from_millis(500));

    adb(&["shell", "input", "keyevent", "KEYCODE_DPAD_CENTER"]);
    sleep(Duration::from_millis(500));
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