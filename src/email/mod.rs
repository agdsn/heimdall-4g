use std::env;
use async_channel::{unbounded, RecvError, Receiver};
use anyhow::{Result, bail};
use lettre::{Address, Message, SmtpTransport, Transport};
use lettre::message::header::ContentType;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use log::{error, info};
use serde::{Deserialize, Serialize};
use crate::email;
use crate::modem::SMS;

pub struct EmailSender {
    email: String,
    recipient: String,
    mailer: SmtpTransport,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SMTPCredentials {
    pub email: String,
    pub server: String,
    pub password: String,
}

impl SMTPCredentials {
    pub fn new(email: String, server: String, password: String) -> Self {
        Self {
            email: email,
            server: server,
            password: password,
        }
    }
}

impl EmailSender {
    pub fn new(email: String, server: String, recipient: String, smtp_user: String, smtp_password: String) -> Result<EmailSender> {
        info!("Loging into mail Server {} with username {}", smtp_user, smtp_password);
        let creds = Credentials::new(email.clone(), smtp_password.to_owned());

        let mailer = SmtpTransport::relay(&server)?
            .credentials(creds).port(465)
            .build();

        Ok(EmailSender {email, recipient, mailer })
    }
    pub async fn send_sms(&self, sender_number: &str, content: &str, time_stamp: &str) -> Result<()> {
        let email = Message::builder()
            .from(Mailbox::new(Some("Scotty".to_owned()), self.email.parse()?))
            .to(Mailbox::new(Some("List".to_owned()), self.recipient.parse()?))
            .subject(format!("Neue SMS von {} empfangen um {}", sender_number, time_stamp))
            .header(ContentType::TEXT_PLAIN)
            .body(String::from(content))?;
        //info!("Sending SMS to {}", self.server.as_str());

        

        self.mailer.send(&email)?;
        Ok(())
    }
}


pub async fn mail_task(mut reader: Receiver<SMS>) -> Result<()> {
    loop {
        let sms = reader.recv().await?;
        info!("Trying to send an sms: {:?}", sms.data);
        let email = EmailSender::new(env::var("EMAIL")?, env::var("SERVER")?, env::var("RECIPIENT")?, env::var("SMTP_USER")?, env::var("PASSWORD")?)?;
        
        match email.send_sms(&sms.send_number, &sms.data, &sms.time).await {
            Ok(_) => info!("Email send"),
            Err(e) => error!("{}", e),
        };

    }
}

pub async fn send_mail_task(mut reader: Receiver<Message>) -> Result<()> {
    loop {
        let message = reader.recv().await?;
        //info!("Trying to send an sms: {:?}", sms.data);
        //let email = EmailSender::new(env::var("EMAIL")?, env::var("SERVER")?, env::var("RECIPIENT")?, env::var("SMTP_USER")?, env::var("PASSWORD")?)?;

        let creds = Credentials::new(env::var("EMAIL")?, env::var("PASSWORD")?);

        let mailer = SmtpTransport::relay(&*env::var("SERVER")?)?
            .credentials(creds).port(465)
            .build();

        match mailer.send(&message) {
            Ok(_) => info!("Email send"),
            Err(e) => error!("{}", e),
        };
    }
}
