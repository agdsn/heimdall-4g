use std::env;
use std::sync::Arc;
use async_channel::{Receiver, Sender};
use lettre::{Message, Address};
use lettre::message::header::ContentType;
use lettre::message::Mailbox;
use anyhow::{Result, bail};
use log::{error, info};
use rusqlite::Connection;
use crate::modem::SMS;


pub trait Gateway: Send {
    fn generate_sms(&self, sms: SMS) -> Option<Message>;
    fn generate_email(sender: Mailbox, receivers: Vec<Mailbox>, header: String, body: String) -> Result<Message> where Self: Sized {
        
        if receivers.is_empty() {
            bail!("No receivers are specified")
        }

        let mut msg_build = Message::builder()
            .from(sender)
            .subject(header)
            .header(ContentType::TEXT_PLAIN);
        
        for receiver in receivers {
            msg_build = msg_build.to(receiver);
        }
        let msg = msg_build.body(body)?;
        anyhow::Ok(msg)
    }
    fn gen_header(&self, sms: &SMS, content: &str) -> String {
        content.replace("%DATE", sms.time.as_str()).replace("%FROM", sms.send_number.as_str())
    }
}


pub struct DefaultRouter {
    mailbox_sender: Mailbox,
    mailbox_receiver: Mailbox,
    content: String,
}

impl DefaultRouter {
    pub fn new(mailbox_sender: Mailbox, mailbox_receiver: Mailbox) -> DefaultRouter {
        if let Ok(template) = env::var("TEMPLATE"){
            return DefaultRouter {mailbox_sender, mailbox_receiver, content: template}
        }
        DefaultRouter {mailbox_sender, mailbox_receiver, content: String::from("Neue SMS von %FROM empfangen um %DATE")}
    }
}

impl Gateway for DefaultRouter {
    fn generate_sms(&self, sms: SMS) -> Option<Message> {
        match DefaultRouter::generate_email(self.mailbox_sender.clone(), vec![self.mailbox_receiver.clone()], self.gen_header(&sms, &self.content), sms.data.to_string()) {
            Ok(mail) => return Some(mail),
            Err(error) => error!("Unable to create mail: {}", error)
        };
        None
    }
}

pub struct SQLRouter {
    conn: Connection,
    mailbox_sender: Mailbox,
    header: String,
    default_mailbox_receiver: Mailbox
}

impl SQLRouter {
    pub fn new(mailbox_sender: Mailbox, default_mailbox_receiver: Mailbox, path_to_database: &str) -> Result<SQLRouter> {
        let conn = Connection::open(path_to_database)?;

        let stmt: rusqlite::Result<String> = conn.query_one("SELECT content FROM settings WHERE key = 'header'", [], |row| row.get(0));
        

        if let Ok(header) = stmt { 
            anyhow::Ok(SQLRouter {mailbox_sender, default_mailbox_receiver, conn, header})
        } else {
            anyhow::Ok(SQLRouter {mailbox_sender, default_mailbox_receiver, conn, header: String::from("Neue SMS von %FROM empfangen um %DATE")})
        }
    }

    fn get_receiver(&self, number: &str) -> Result<Vec<Mailbox>> {
        let mut stmt = self.conn.prepare("SELECT name, mail FROM Mail WHERE (?1) = number")?;
        
        let mail_list: Vec<(Arc<str>, Arc<str>)>   = stmt
        .query_map([number], |row| {
            
                Ok((row.get(0)?,
                row.get(1)?))
        })?.collect::<Result<Vec<_>, _>>()?;
        let mut out: Vec<Mailbox> = Vec::new();
        for mail in mail_list {
            match mail.0.to_string().parse::<Address>() {
                Ok(mail_addr) => out.push(Mailbox::new(Some(mail.1.to_string()), mail_addr)),
                Err(e) => error!("Unable to parse Mail address: {} exited with error {}", mail.0, e)
            };
        }
        Ok(out)
    }

}

impl Gateway for SQLRouter {
    fn generate_sms(&self, sms: SMS) -> Option<Message> {
        
        let receivers = self.get_receiver(&sms.send_number).unwrap_or(vec![self.default_mailbox_receiver.clone()]);

        match DefaultRouter::generate_email(self.mailbox_sender.clone(), receivers, self.gen_header(&sms, &self.header), sms.data.to_string()) {
            Ok(mail) => return Some(mail),
            Err(error) => error!("Unable to create mail: {}", error)
        };
        None
    }
}


pub async fn bifroest(scotty: Box<dyn Gateway>, reader: Receiver<SMS>, sender: Sender<Message>)
{
    info!("Enterprise on command!");
    loop {
        let sms = match reader.recv().await {
            Ok(sms) => sms,
            Err(e) => {error!("unable to receive Data from modem! {}", e); panic!("Unable to receive");}
        };

        if let Some(email) = scotty.generate_sms(sms) {
            match sender.send(email).await {
                Ok(()) => info!("Sending Mail"),
                Err(e) => error!("Unable to send to mail thread: {}", e)
            };
        };
    }
}
