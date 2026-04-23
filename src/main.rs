use async_channel::unbounded;
use dotenv::dotenv;
use heimdall::Config;
use heimdall::modem::modem_loop;
use heimdall::email::send_mail_task;
use heimdall::router::bifroest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn")
    ).init();

    let config = Config::default();

    let (s, r) = unbounded();
    let (s2, r2) = unbounded();


    let task = tokio::spawn(async move { modem_loop(config.serial_port, s).await });
    let task_router = tokio::spawn(async move { bifroest(config.router, r, s2).await });
    let task2 = tokio::spawn(async move { send_mail_task(r2).await });
    match tokio::try_join!(task, task_router, task2) {
        Ok(_) => Ok(()),
        Err(e) => { panic!("Should not panic: {}", e) },
    }
}