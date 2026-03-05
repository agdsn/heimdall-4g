use async_channel::unbounded;
use dotenv::dotenv;
use heimdal_4g::{Modem, Config, EmailSender, };
use log::{info, error};
use serialport::SerialPort;
use heimdal_4g::modem::modem_loop;
use heimdal_4g::email::mail_task;
use heimdal_4g::router::DefaultRouter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).init();
    let config = Config::default();
    info!("Starting modem project with config: {:?}", config);

    let (s, r) = unbounded();
    let mut modem = Modem::new(config.serial_port.as_str(), config.baud_rate)?;

    let task = tokio::spawn(async move { modem_loop(&mut modem, s).await });
    let task2 = tokio::spawn(async move { mail_task(r).await });
    tokio::try_join!(task, task2)?;

    // let mut modem = Modem::new(config);

    /*match modem.connect().await {
        Ok(_) => info!("Modem connected successfully"),
        Err(e) => {
            error!("Failed to connect modem: {}", e);
            return Err(e);
        }
    }*/

    // Your main application logic here
    Ok(())
}