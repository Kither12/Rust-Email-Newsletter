use mailin_embedded::{Handler, Server, SslConfig};
use std::net::TcpListener;
use std::net::ToSocketAddrs;
#[derive(Default, Clone)]
struct MailHandler {}
impl Handler for MailHandler {}

pub fn open_smtp_sever<A: ToSocketAddrs>(addr: A) -> Result<(), mailin_embedded::err::Error> {
    let handler = MailHandler::default();
    let mut server = Server::new(handler);
    let listener = TcpListener::bind(&addr)?;

    let name = env!("CARGO_PKG_NAME");
    server
        .with_name(name)
        .with_ssl(SslConfig::None)?
        .with_addr(addr)?
        .with_tcp_listener(listener);
    std::thread::spawn(|| {
        server.serve().expect("Failed to start sever");
    });
    Ok(())
}
