mod dummy_serial;

use std::{env, io};
use std::io::{Read, Write};
use std::thread::sleep;
use std::time::Duration;
use at_commands::builder::CommandBuilder;
use at_commands::parser::CommandParser;
use serialport::{ClearBuffer, SerialPort};
use anyhow::{Result, bail, Error};
use async_channel::Sender;
use thiserror::Error;
use log::{info, warn, error};
use crate::config::SerialPortConfig;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("Sim seems to be not ready")]
pub struct SimNotReady;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("Sim seems to be not ready")]
pub struct UnparsableATResponse;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("Sim seems to be not ready")]
pub struct UnableToSetConfig;


enum  Strategy {
    PULL,
    WAIT,
}

pub struct SMS {
    pub received_number: String,
    pub send_number: String,
    pub time: String,
    pub data: String,
    pub message_type: String,
}

pub struct ModemConfig {
    pub sms_mode: i32,
    pub sms_charset: String,
}

pub enum ModemStatus {
    SMS(i32),
    SIM_STAT(u8),
    NONE,
}

pub struct Modem {
    write_buffer: Vec<u8>,
    read_buffer: Vec<u8>,
    serial: Box<dyn SerialPort>,
}

impl Modem
{
    pub fn new(port: &str, baud: u32) -> Result<Modem> {
        let mut port  = serialport::new(port, baud).open()?;
        port.set_timeout(Duration::from_millis(5000))?;
        Ok(Modem{write_buffer: vec![], read_buffer: vec![], serial: port})
    }

    async fn send(&mut self, timeout: u16) {
        self.serial.write(self.write_buffer.as_slice()).unwrap();
        self.serial.write(b"\n\r").unwrap();

        self.write_buffer.clear();
        // insert sleep here
        tokio::time::sleep(Duration::from_millis(timeout as u64)).await;
        self.read_buffer.clear();
        self.read_until_delim(b"OK").unwrap();
    }

    async fn set_config(&mut self, timeout: u16) -> Result<()> {
        self.send(timeout).await;
        Ok(())
        /*match CommandParser::parse(&self.read_buffer).expect_identifier(b"OK").finish() {
            Ok(_) => {Ok(())},
            Err(_) => {bail!(UnableToSetConfig)}
        }*/
    }

    async fn check_power(&mut self) -> Result<()> {
        self.write_buffer.extend_from_slice(b"ATI\r\n");

        self.send(300).await;
        return Ok(());
    }

    pub async fn online(&mut self)-> Result<bool> {
        self.write_buffer.extend_from_slice(CommandBuilder::create_query(&mut [0u8; 256], true)
            .named("+CSQ")
            .finish().unwrap());

        self.send(0).await;

        match CommandParser::parse(&self.read_buffer).expect_identifier(b"+CSQ").expect_int_parameter().expect_int_parameter().expect_identifier(b"OK").finish() {
            Ok(resp) => {
                if resp.0 == 99 {
                    return Ok(false)
                }
                Ok(true)
            },
            Err(_) => {bail!(UnparsableATResponse)}
        }
    }

    pub async fn sim_pin(&mut self, pin: &str)-> Result<()> {
        self.write_buffer.extend_from_slice(CommandBuilder::create_set(&mut [0x00; 256], true).named("+CPIN").with_raw_parameter(pin).finish().unwrap());

        self.send(0).await;

         match CommandParser::parse(&self.read_buffer).expect_identifier(b"Ok").finish() {
             Ok(_) => Ok(()),
             Err(_) => bail!(UnparsableATResponse),
         }
    }

    pub async fn check_sim(&mut self)-> Result<()> {
        self.write_buffer.extend_from_slice(CommandBuilder::create_query(&mut [0u8; 256], true).named("+QINISTAT").finish().unwrap());

        self.send(0).await;

        match CommandParser::parse(&self.read_buffer).expect_identifier(b"+QINISTAT").expect_int_parameter().expect_identifier(b"OK").finish() {
            Ok(response) => {
                match response.0 {
                    1 => {bail!(SimNotReady)},
                    2 => {Ok(())}
                    _ => {bail!(SimNotReady)}
                }
            }
            Err(_) => {bail!(UnparsableATResponse)}
        }
    }

    pub async fn read_sms(&mut self, slot: i32) -> Result<SMS> {
        self.write_buffer.extend_from_slice(CommandBuilder::create_set(&mut [0u8; 256], true).named("+CMGR").with_int_parameter(slot).finish().unwrap_or(&[]));
        let s = String::from_utf8(self.write_buffer.clone())
            .expect("Invalid UTF-8 sequence");

        println!("{}", s);
        self.send(1).await;

        let read = self.split_vec_on_crlf();
        //println!("{}",  String::from_utf8(read[2].to_vec()).unwrap());
        match CommandParser::parse(&read[1])
            .expect_identifier(b"+CMGR")
            .expect_string_parameter()
            .expect_string_parameter()
            .expect_optional_int_parameter()
            .expect_string_parameter()
            .finish() {
            Ok(response) => {
                let mut sms = SMS{received_number: response.1.parse()?, send_number: response.1.parse()?, time: response.3.parse()?, data: String::new(), message_type: response.0.parse()?};

                sms.data = String::try_from(read[2].to_vec())?;
                println!("SMS received number: {} and {}", sms.received_number, sms.data);
                Ok(sms)

            },
            Err(_) => {
                info!("Not Parsed");
                bail!(UnparsableATResponse)
            }

        }
    }

    pub async fn check_sms(&mut self)-> Result<Vec<SMS>> {
        self.read_until_delim(b"OK")?;
        let mut out = Vec::<SMS>::new();
        match CommandParser::parse(&self.read_buffer).expect_identifier(b"+CMTI").expect_raw_string().expect_int_parameter().finish() {
            Ok(resp) => {
                if resp.0 == "SM" {
                    out.push(self.read_sms(resp.1).await?);
                }
            },
            Err(_) => {bail!(UnparsableATResponse)}
        }

        Ok(out)

    }

    pub async fn send_sms(&mut self, address: &str, content: &str) -> Result<()> {
        self.write_buffer.extend_from_slice(CommandBuilder::create_set(&mut [0x00; 4096], true).named("+CMGS").with_string_parameter(address).with_raw_parameter(content.as_bytes()).finish_with(b"\x1A").unwrap());
        self.send(120).await;

        match CommandParser::parse(&self.read_buffer).expect_identifier(b"+CMGS").expect_identifier(b"OK").finish() {
            Ok(_) => Ok(()),
            Err(_) => {bail!(UnparsableATResponse)}
        }
    }

    pub async fn delete_sms(&mut self, id: i32)-> Result<()> {
        self.write_buffer.extend_from_slice(CommandBuilder::create_set(&mut [0x00; 4096], true).named("+CMGD").with_int_parameter(id).finish().unwrap_or(&[]));
        self.send(120).await;
        Ok(())
    }

    pub async fn load_config(&mut self, config: ModemConfig) -> Result<()> {
        self.write_buffer.extend_from_slice(CommandBuilder::create_set(&mut [0x00; 256], true).named("+CMGF").with_int_parameter(config.sms_mode).finish().unwrap());
        self.set_config(1).await?;

        //self.write_buffer.extend_from_slice(CommandBuilder::create_set(&mut [0x00; 256], true).named("+CSCS").with_string_parameter(config.sms_charset).finish().unwrap());
        //self.set_config(1)?;

        Ok(())
    }

    fn split_vec_on_crlf(&mut self) -> Vec<&[u8]> {
        let mut parts = Vec::new();
        let mut start = 0;

        for i in 0..self.read_buffer.len()-1 {
            if self.read_buffer[i] == b'\r' && self.read_buffer[i+1] == b'\n' {
                if start == i {
                    continue;
                }
                parts.push(&self.read_buffer[start..i]);
                start = i + 2;
            }
        }

        if start < self.read_buffer.len() {
            parts.push(&self.read_buffer[start..]);
        }

        parts
    }

    fn read_until_delim(&mut self, delim: &[u8]) -> io::Result<()> {
        let mut result = Vec::new();
        let mut buf = [0u8; 128];

        loop {
            if delim.is_empty() {
                break;
            }
            match self.serial.read(&mut buf) {
                Ok(0) => { info!("nothing to be rad") },
                Ok(n) => {
                    result.extend_from_slice(&buf[..n]);
                    if result.windows(delim.len()).any(|w| w == delim){
                        break;
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                    info!("Timed out");
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        self.read_buffer = result;


        Ok(())
    }

    pub fn read(&mut self) -> Result<ModemStatus> {
        if self.serial.bytes_to_read()? == 0 {
            return Ok(ModemStatus::NONE)
        }

        self.read_until_delim(b"\r")?;

        info!("read: {:?}", self.read_buffer);
        if CommandParser::parse(&self.read_buffer).expect_identifier(b"\r\nAPP RDY\r\n").finish().is_ok() {
            return Ok(ModemStatus::NONE);
        }
        if let Ok(value) = CommandParser::parse(&self.read_buffer).expect_identifier(b"\r\n+CMTI: ").expect_string_parameter().expect_int_parameter().expect_identifier(b"\r\n").finish() {
            return Ok(ModemStatus::SMS(value.1))
        }
        let s = String::from_utf8(self.read_buffer.clone())
            .expect("Invalid UTF-8 sequence");

        println!("{}", s);
        warn!("Unknown modem response: {:?}", self.read_buffer);
        Ok(ModemStatus::NONE)
    }

}

impl Drop for Modem {
    fn drop(&mut self) {

        info!("Dropping Modem");
    }
}

async fn pulling(modem: &mut Modem, sender: &Sender<SMS>) -> Result<()> {
    modem.load_config(ModemConfig { sms_mode: 1, sms_charset: String::new() }).await?;

    for i in 1..=23 {
        let sms_res = modem.read_sms(i).await;
        if let Ok(sms) = sms_res {
            info!("SMS ready to beam from: {}  {} on {}", sms.send_number, sms.data, sms.time);

            sender.send(sms).await?;
            modem.delete_sms(i).await?;
        }
    }

    Ok(())
}

async fn waiting(modem: &mut Modem, sender: &Sender<SMS>) -> Result<()> {
    match modem.read() {
        Ok(ModemStatus::NONE) => info!("Modem continue!"),
        Ok(ModemStatus::SMS(status)) => {
            info!("Received SMS in slot: {:?}", status);
            //modem.load_config(ModemConfig{sms_mode: 1,  sms_charset: String::new()}).await?;

            let sms = modem.read_sms(status).await?;
            info!("SMS ready to beam from: {}  {} on {}", sms.send_number, sms.data, sms.time);

            sender.send(sms).await?;
            modem.delete_sms(status).await?;
        }
        Err(e) => {
            error!("Unknown modem response {}",e);
            let sms = SMS{received_number: "Error".parse()?, send_number: "Error".parse()?, time: "Now".parse()?, data: format!("There was a error while writing: {}", e), message_type: "Error".parse()? };
            sender.send(sms).await?;
            bail!(UnparsableATResponse);
        },
        _ => info!("Modem continue!"),
    }
    modem.check_power().await?;
    Ok(())
}

pub async fn modem_loop(serial_port: SerialPortConfig, sender: Sender<SMS>) -> Result<()> {
    let pull_time = match env::var("PULL_TIME") {
        Ok(t) => t.parse::<u64>().unwrap_or(300000),
        Err(_) => {
            info!("Set PULL_TIME to 30000ms");
            300000
        },
    };

    let strategy = match env::var("MODEM_STRAT") {
        Ok(ref v) if v == "WAIT" => Strategy::WAIT,
        _ => Strategy::PULL,
    };

    let mut modem = Modem::new(serial_port.serial_port.as_str(), serial_port.baud_rate)?;
    modem.load_config(ModemConfig { sms_mode: 1, sms_charset: String::new() }).await?;
    pulling(&mut modem, &sender).await?;

    loop {

        match strategy {
            Strategy::WAIT => {
                waiting(&mut modem, &sender).await?;
            }
            Strategy::PULL => {
                pulling(&mut modem, &sender).await?;
            }
        }
        tokio::time::sleep(Duration::from_millis(pull_time)).await;
   }
}
