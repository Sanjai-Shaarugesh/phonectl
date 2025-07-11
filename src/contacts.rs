use std::io::{self, Write};
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use regex::Regex;
use crate::adb::adb;
use crate::audio::start_audio_routing;
use std::process::Stdio;

#[derive(Clone, Debug)]
pub struct Contact {
    pub name: String,
    pub number: String,
}

pub fn get_contacts() -> Vec<Contact> {
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

pub fn list_contacts() {
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

pub fn search_prompt(query: &str) {
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

pub fn call_prompt(query: &str) {
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

pub fn dial_prompt(query: &str) {
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

pub fn answer_call() {
    println!("📞 Answering call...");

    adb(&["shell", "input", "keyevent", "KEYCODE_HEADSETHOOK"]);
    sleep(Duration::from_millis(500));

    adb(&["shell", "input", "keyevent", "KEYCODE_CALL"]);
    sleep(Duration::from_millis(500));

    let screen_info = crate::adb::adb_output(&["shell", "dumpsys", "window", "displays"]);
    let (width, height) = crate::unlock::parse_screen_dimensions(&screen_info);

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
        "1000",
    ]);

    println!("✅ Call answered");
}

pub fn end_call() {
    println!("📞 Ending call...");

    let methods = [
        ("KEYCODE_ENDCALL", "Standard end call"),
        ("KEYCODE_HEADSETHOOK", "Headset hook"),
        ("KEYCODE_BACK", "Back button"),
    ];

    for (keycode, description) in methods.iter() {
        println!("- Trying {} method...", description);
        let success = Command::new("adb")
            .args(&["shell", "input", "keyevent", keycode])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        sleep(Duration::from_millis(500));

        if success {
            println!("✅ Call ended using {}", description);
            return;
        }
    }

    println!("⚠️ Failed to end call. Phone might already be idle.");
}