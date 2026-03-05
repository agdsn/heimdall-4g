use std::env;
use async_channel::{unbounded, RecvError, Receiver};
use anyhow::{Result, bail};
use lettre::{Address, Message, SmtpTransport, Transport};
use lettre::message::header::ContentType;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use log::{error, info};
use crate::email;
use crate::modem::SMS;

pub struct EmailSender {
    email: String,
    server: String,
    recipient: String,
    credentials: Credentials,
}

pub struct EmailReceiver {
    email: Address,
    name: String
}

impl EmailReceiver {
    pub fn new(email: String, name: String) -> Result<Self> {
        let email:Address = email.parse()?;
        Ok(EmailReceiver { email, name })
    }

    pub fn get_mailbox(self) -> Mailbox {
        Mailbox::new(Some(self.name), self.email)
    }
}

impl EmailSender {
    pub fn new(email: String, server: String, recipient: String, smtp_user: String, smtp_password: String) -> EmailSender {
        println!("username {}, password: {}", smtp_user, smtp_password);
        let creds = Credentials::new(email.clone(), smtp_password.to_owned());

        EmailSender {email, server, recipient, credentials: creds }
    }
    pub async fn send_sms(&self, sender_number: &str, content: &str, time_stamp: &str) -> Result<()> {
        let email = Message::builder()
            .from(Mailbox::new(Some("Scotty".to_owned()), self.email.parse()?))
            .to(Mailbox::new(Some("List".to_owned()), self.recipient.parse()?))
            .subject(format!("Neue SMS von {} empfangen um {}", sender_number, time_stamp))
            .header(ContentType::TEXT_PLAIN)
            .body(String::from(content))?;
        info!("Sending SMS to {}", self.server.as_str());

        let mailer = SmtpTransport::relay("smtp.agdsn.de")?
            .credentials(self.credentials.clone()).port(465)
            .build();

        mailer.send(&email)?;
        Ok(())
    }
}


pub async fn mail_task(mut reader: Receiver<SMS>) -> Result<()> {
    loop {
        let sms = reader.recv().await?;
        info!("Trying to send an sms: {:?}", sms.data);
        let email = EmailSender::new(env::var("EMAIL")?, env::var("SERVER")?, env::var("RECIPIENT")?, env::var("SMTP_USER")?, env::var("PASSWORD")?);

        match email.send_sms(&sms.send_number, &sms.data, &sms.time).await {
            Ok(_) => info!("Email send"),
            Err(e) => error!("{}", e),
        };

    }
}