use std::path::PathBuf;
use dirs;

pub const CONFIG_FILE: &str = ".phonectl_auth";
pub const DEVICE_FILE: &str = ".phonectl_devices";
pub const KEY_FILE: &str = ".phonectl_key";
pub const AUDIO_FORWARD_PORT: &str = "28200";

pub fn get_config_path() -> PathBuf {
    dirs::home_dir().unwrap().join(CONFIG_FILE)
}

pub fn get_device_file_path() -> PathBuf {
    dirs::home_dir().unwrap().join(DEVICE_FILE)
}

pub fn get_key_file_path() -> PathBuf {
    dirs::home_dir().unwrap().join(KEY_FILE)
}