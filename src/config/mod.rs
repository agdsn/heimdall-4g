use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub serial_port: String,
    pub baud_rate: u32,
    pub timeout_ms: u64,
    pub debug: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            serial_port: "/dev/ttyUSB3".to_string(),
            baud_rate: 115200,
            timeout_ms: 5000,
            debug: false,
        }
    }
}