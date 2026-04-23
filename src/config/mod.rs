use std::env;
use lettre::message::Mailbox;
use rusqlite::fallible_iterator::FallibleIterator;
use serde::{Deserialize, Serialize};
use crate::DefaultRouter;
use crate::email::SMTPCredentials;
use crate::router::{Gateway, SQLRouter};

pub struct Config {
    pub serial_port: SerialPortConfig,
    pub smtp_cred: SMTPCredentials,
    pub debug: bool,
    pub router: Box<dyn Gateway>
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerialPortConfig {
    pub serial_port: String,
    pub baud_rate: u32,
    pub timeout_ms: u64,
}

impl Default for SerialPortConfig {
    fn default() -> Self {
        Self {
            serial_port: "/dev/ttyUSB3".to_string(),
            baud_rate: 115200,
            timeout_ms: 5000,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        if env::var("EMAIL").is_err() || env::var("SERVER").is_err() || env::var("PASSWORD").is_err() || env::var("ROUTER").is_err(){
            panic!("EMAIL and SERVER and PASSWORD for SMTP Server must be set!");
        }


        Self {
            serial_port: SerialPortConfig::default(),
            debug: false,
            smtp_cred: SMTPCredentials::new(env::var("EMAIL").unwrap(), env::var("SERVER").unwrap(),env::var("PASSWORD").unwrap()),
            router: create_gateway()
        }

    }

}
fn create_gateway() -> Box<dyn Gateway + Send> {
    let sender = Mailbox::new(match env::var("SENDER_NAME") { Ok(x)=> Some(x), _ => None }, env::var("EMAIL").unwrap().parse().unwrap());
    match env::var("ROUTER").unwrap().as_str() {
        "default" => {
            if let Ok(receiver) = env::var("RECIPIENT") {
                let receiver = Mailbox::new(match env::var("SENDER_NAME") { Ok(x)=> Some(x), _ => None }, receiver.parse().unwrap());
                Box::new(DefaultRouter::new(sender, receiver))

            } else {
                panic!("RECIPIENT environment variable must be set!");
            }

        }
        "sql" => {
            if let Ok(backend) = env::var("SQL_BACKEND") {
                Box::new(SQLRouter::new(sender, &backend).unwrap())
            } else {
                panic!("SQL_BACKEND environment variable must be set!");
            }
        }
        e => panic!("Unkown ROUTER: {}", e),
    }
}