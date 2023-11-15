use base64::engine::general_purpose;
use base64::Engine;
use chrono::{DateTime, Local};
use mail_parser::MimeHeaders;
use mailin_embedded::{Server, SslConfig};
use serde::{Deserialize, Serialize};
use std::net::ToSocketAddrs;
use std::sync::{Arc, RwLock};
use std::collections::HashSet;

/*
The main purpose of SMTP sever is just for unit testing, but I still cover most of features that a mail sever should have
*/

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct Address {
    name: Option<String>,
    email: Option<String>,
}

impl From<&mail_parser::Addr<'_>> for Address {
    fn from(addr: &mail_parser::Addr) -> Self {
        Address {
            name: addr.name.clone().map(|v| v.to_string()),
            email: addr.address.clone().map(|v| v.to_string()),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, Hash)]
pub struct Attachment {
    filename: String,
    content_id: Option<String>,
    mime: String,
    size: String,
    content: String,
}

impl From<&mail_parser::MessagePart<'_>> for Attachment {
    fn from(part: &mail_parser::MessagePart) -> Self {
        let filename = part.attachment_name().unwrap_or_default().to_string();
        let mime = match part.content_type() {
            Some(content_type) => match &content_type.c_subtype {
                Some(subtype) => format!("{}/{}", content_type.c_type, subtype),
                None => content_type.c_type.to_string(),
            },
            None => "application/octet-stream".to_owned(),
        };

        Attachment {
            filename,
            mime,
            content_id: part.content_id().map(|s| s.to_owned()),
            size: humansize::format_size(part.contents().len(), humansize::DECIMAL),
            content: general_purpose::STANDARD.encode(part.contents()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Default, PartialEq, Eq, Hash)]
pub struct MailMessage {
    from: Address,
    to: Vec<Address>,
    subject: String,
    date: String,
    size: String,
    opened: bool,
    text: String,
    html: String,
    attachments: Vec<Attachment>,
    raw: String,
    pub envelope_from: String,
    pub envelope_recipients: Vec<String>,
}

impl TryFrom<mail_parser::Message<'_>> for MailMessage {
    type Error = &'static str;

    fn try_from(message: mail_parser::Message) -> Result<Self, Self::Error> {
        let from = match message.from().and_then(|address| address.first()) {
            Some(addr) => addr.into(),
            _ => {
                tracing::warn!(
                    "Could not parse 'From' address header, setting placeholder address."
                );

                Address {
                    name: Some("No from header".to_string()),
                    email: Some("no-from-header@example.com".to_string()),
                }
            }
        };
        let to = match message
            .from()
            .and_then(|address| Some(address.clone().into_list()))
        {
            Some(addr) => addr
                .iter()
                .map(|addr| addr.into())
                .collect::<Vec<Address>>(),
            _ => {
                tracing::warn!("Could not parse 'To' address header, setting placeholder address.");
                vec![Address {
                    name: Some("No to header".to_string()),
                    email: Some("no-to-header@example.com".to_string()),
                }]
            }
        };

        let subject = message.subject().unwrap_or_default().to_owned();

        let text = match message
            .text_bodies()
            .find(|p| p.is_text() && !p.is_text_html())
        {
            Some(item) => item.to_string(),
            _ => Default::default(),
        };

        let html = match message.html_bodies().find(|p| p.is_text_html()) {
            Some(item) => item.to_string(),
            _ => Default::default(),
        };

        let attachments = message
            .attachments()
            .map(|attachement| attachement.into())
            .collect::<Vec<Attachment>>();

        let date: DateTime<Local> = match message.date() {
            Some(date) => match DateTime::parse_from_rfc2822(date.to_rfc3339().as_str()) {
                Ok(date_time) => date_time.into(),
                _ => Local::now(),
            },
            None => Local::now(),
        };

        let raw = general_purpose::STANDARD.encode(&message.raw_message);

        let size = humansize::format_size(message.raw_message.len(), humansize::DECIMAL);

        Ok(MailMessage {
            from,
            to,
            subject,
            date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
            size,
            text,
            html,
            opened: false,
            attachments,
            raw,
            ..MailMessage::default()
        })
    }
}

#[derive(Clone, Debug)]
struct MailHandler {
    buffer: Vec<u8>,
    envelope_from: String,
    envelope_recipients: Vec<String>,
    storage: Arc<RwLock<HashSet<MailMessage>>>
}

impl MailHandler {
    fn create(storage: Arc<RwLock<HashSet<MailMessage>>>) -> Self {
        MailHandler {
            buffer: Vec::new(),
            envelope_from: String::new(),
            envelope_recipients: Vec::new(),
            storage
        }
    }
}

impl MailHandler {
    fn parse_mail(&mut self) -> Result<(), &'static str> {
        let parsed = mail_parser::MessageParser::default()
            .parse(&self.buffer)
            .ok_or("Could not parse email using mail_parser")?;
        let mut message: MailMessage = parsed.try_into()?;
        message.envelope_from = std::mem::take(&mut self.envelope_from);
        message.envelope_recipients = std::mem::take(&mut self.envelope_recipients);

        self.buffer.clear();
        (*self.storage.write().expect("Faild to write into storage")).insert(message);
        Ok(())
    }
}

impl mailin_embedded::Handler for MailHandler {
    fn mail(
        &mut self,
        _ip: std::net::IpAddr,
        _domain: &str,
        from: &str,
    ) -> mailin_embedded::Response {
        self.envelope_from = from.to_string();
        mailin_embedded::response::OK
    }
    fn rcpt(&mut self, to: &str) -> mailin_embedded::Response {
        self.envelope_recipients.push(to.to_string());
        mailin_embedded::response::OK
    }

    fn data(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.buffer.extend_from_slice(buf);
        Ok(())
    }

    fn data_end(&mut self) -> mailin_embedded::Response {
        match self.parse_mail() {
            Err(e) => {
                tracing::error!("Error parsing email: {}", e);

                mailin_embedded::response::Response::custom(
                    500,
                    "Error parsing message".to_string(),
                )
            }
            Ok(_) => mailin_embedded::response::OK,
        }
    }
}

pub fn open_smtp_sever<A: ToSocketAddrs>(
    addr: A,
    storage: Arc<RwLock<HashSet<MailMessage>>>,
) -> Result<(), mailin_embedded::err::Error> {
    let mail_handler = MailHandler::create(storage);
    let mut server = Server::new(mail_handler);

    let name = env!("CARGO_PKG_NAME");
    server
        .with_name(name)
        .with_ssl(SslConfig::None)?
        .with_addr(addr)?;
    std::thread::spawn(|| {
        server.serve().expect("Failed to start sever");
    });
    Ok(())
}
