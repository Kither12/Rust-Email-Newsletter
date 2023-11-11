use base64::Engine;
use mailin_embedded::{Handler, Response, Server, SslConfig};
use std::convert::Infallible;
use std::{collections::HashSet, sync::Arc};
use std::net::ToSocketAddrs;
use std::sync::RwLock;
use uuid::Uuid;
use chrono::{DateTime, Local};
use mail_parser::MimeHeaders;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use base64::engine::general_purpose;
use tokio::sync::broadcast::{Sender, Receiver};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};
pub type MessageId = Uuid;

/*
The main purpose of SMTP sever is just for unit testing, but I still cover most of features that a mail sever should have
*/

#[derive(Deserialize, Debug)]
pub enum Action {
    RemoveAll,
    Remove(MessageId),
    Open(MessageId),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttachmentMetadata {
    filename: String,
    mime: String,
    size: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MailMessageMetadata {
    pub id: MessageId,
    from: Address,
    to: Vec<Address>,
    subject: String,
    pub time: i64,
    date: String,
    size: String,
    opened: bool,
    pub has_html: bool,
    pub has_plain: bool,
    pub attachments: Vec<AttachmentMetadata>,
    pub envelope_from: String,
    pub envelope_recipients: Vec<String>,
}

impl From<MailMessage> for MailMessageMetadata {
    fn from(message: MailMessage) -> Self {
        let MailMessage {
            id,
            from,
            to,
            subject,
            time,
            date,
            size,
            html,
            text,
            opened,
            attachments,
            envelope_from,
            envelope_recipients,
            ..
        } = message;
        MailMessageMetadata {
            id,
            from,
            to,
            subject,
            time,
            date,
            size,
            has_html: !html.is_empty(),
            has_plain: !text.is_empty(),
            opened,
            attachments: attachments
                .into_iter()
                .map(|a| AttachmentMetadata {
                    filename: a.filename,
                    mime: a.mime,
                    size: a.size,
                })
                .collect::<Vec<AttachmentMetadata>>(),
            envelope_from,
            envelope_recipients,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
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

#[derive(Clone, Debug, Serialize)]
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

#[derive(Clone, Debug, Serialize, Default)]
pub struct MailMessage {
    pub id: MessageId,
    pub time: i64,
    from: Address,
    to: Vec<Address>,
    subject: String,
    date: String,
    size: String,
    opened: bool,
    headers: HashMap<String, String>,
    text: String,
    html: String,
    attachments: Vec<Attachment>,
    raw: String,
    pub envelope_from: String,
    pub envelope_recipients: Vec<String>,
}
impl MailMessage {
    pub fn open(&mut self) {
        self.opened = true;
    }

    pub fn render(&self) -> String {
        if self.html.is_empty() {
            self.text.clone()
        } else {
            let mut html = self.html.clone();

            for attachement in &self.attachments {
                if let Some(content_id) = &attachement.content_id {
                    let from = format!("cid:{}", content_id.trim_start_matches("cid:"));
                    let encoded: String = attachement.content.chars().filter(|c| !c.is_whitespace()).collect();
                    let to = format!("data:{};base64,{}", attachement.mime, encoded);

                    html = html.replace(&from, &to);
                }
            }

            html
        }
    }
}

impl TryFrom<mail_parser::Message<'_>> for MailMessage {
    type Error = &'static str;

    fn try_from(message: mail_parser::Message) -> Result<Self, Self::Error> {
        let from = match message.from().and_then(|address| address.first()) {
            Some(addr) => addr.into(),
            _ => {
                tracing::warn!("Could not parse 'From' address header, setting placeholder address.");

                Address {
                    name: Some("No from header".to_string()),
                    email: Some("no-from-header@example.com".to_string()),
                }
            }
        };
        let to = match message.from().and_then(|address| Some(address.clone().into_list())) {
            Some(addr) => addr.iter().map(|addr| addr.into()).collect::<Vec<Address>>(),
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

        let mut headers = HashMap::<String, String>::new();

        for (key, value) in message.headers_raw() {
            headers.insert(key.to_string(), value.to_string());
        }

        let size = humansize::format_size(message.raw_message.len(), humansize::DECIMAL);

        Ok(MailMessage {
            id: Uuid::new_v4(),
            from,
            to,
            subject,
            time: date.timestamp(),
            date: date.format("%Y-%m-%d %H:%M:%S").to_string(),
            size,
            text,
            html,
            opened: false,
            attachments,
            raw,
            headers,
            ..MailMessage::default()
        })
    }
}

pub struct AppState {
    rx: Receiver<MailMessage>,
    storage: RwLock<HashMap<MessageId, MailMessage>>,
    prefix: String,
    index: Option<String>,
}

async fn storage(
    mut storage_rx: Receiver<MailMessage>,
    state: Arc<AppState>,
    handle: SubsystemHandle,
) -> Result<(), Infallible> {
    let mut running = true;
    while running {
        tokio::select! {
            incoming = storage_rx.recv() => {
                if let Ok(message) = incoming {
                    if let Ok(mut storage) = state.storage.write() {
                        storage.insert(message.id, message);
                    }
                }
            },
            _ = handle.on_shutdown_requested() => {
                running = false;
            },
        }
    }

    Ok(())
}

#[derive(Clone, Debug)]
struct MailHandler {
    tx: Sender<MailMessage>,
    buffer: Vec<u8>,
    envelope_from: String,
    envelope_recipients: Vec<String>,
}

impl MailHandler {
    fn create(tx: Sender<MailMessage>) -> Self {
        MailHandler {
            tx,
            buffer: Vec::new(),
            envelope_from: String::new(),
            envelope_recipients: Vec::new(),
        }
    }
}

impl MailHandler {
    fn parse_mail(&mut self) -> Result<MailMessage, &'static str> {
        let parsed = mail_parser::MessageParser::default().parse(&self.buffer)
            .ok_or("Could not parse email using mail_parser")?;
        let mut message: MailMessage = parsed.try_into()?;
        message.envelope_from = std::mem::take(&mut self.envelope_from);
        message.envelope_recipients = std::mem::take(&mut self.envelope_recipients);

        self.buffer.clear();
        self.tx
            .send(message.clone())
            .map_err(|_| "Could not send email to own broadcast channel")?;

        Ok(message)
    }
}

impl mailin_embedded::Handler for MailHandler {
    fn mail(&mut self, _ip: std::net::IpAddr, _domain: &str, from: &str) -> mailin_embedded::Response {
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

                mailin_embedded::response::Response::custom(500, "Error parsing message".to_string())
            }
            Ok(message) => mailin_embedded::response::OK,
        }
    }
}

pub fn open_smtp_sever<A: ToSocketAddrs>(addr: A, tx: Sender<MailMessage>,) -> Result<(), mailin_embedded::err::Error> {
    let mail_handler = MailHandler::create(tx);
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
