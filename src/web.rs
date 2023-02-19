use embassy_net::tcp::{self, TcpSocket};
use embedded_io::asynch::Write;
use esp_println::println;

#[derive(Debug)]
pub enum Error {
    Generic(&'static str),
    Tcp(tcp::Error),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Generic(s) => write!(f, "{}", s),
            Error::Tcp(e) => write!(f, "tcp error: {:?}", e),
        }
    }
}

impl core::error::Error for Error {}

impl From<tcp::Error> for Error {
    fn from(value: tcp::Error) -> Self {
        Self::Tcp(value)
    }
}

type Result<T> = core::result::Result<T, Error>;

async fn send_static_gzip(socket: &mut TcpSocket<'_>, data: &[u8]) -> Result<()> {
    socket
        .write_all(
            b"HTTP/1.0 200 OK\r\nContent-type: image/svg+xml\r\nContent-Encoding: gzip\r\n\r\n",
        )
        .await?;
    socket.write_all(data).await?;
    Ok(())
}

async fn send_static(socket: &mut TcpSocket<'_>, data: &[u8]) -> Result<()> {
    socket.write_all(b"HTTP/1.0 200 OK\r\n\r\n").await?;
    socket.write_all(data).await?;
    Ok(())
}

pub async fn handle_connection(task_n: u32, socket: &mut TcpSocket<'_>) -> Result<()> {
    let mut buffer = [0u8; 1024];

    // read all headers
    let mut offset = 0;
    loop {
        let read_len = socket.read(&mut buffer[offset..]).await?;
        offset += read_len;
        if buffer[..offset].ends_with(b"\r\n\r\n") {
            break;
        }
    }

    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Request::new(&mut headers);
    if !req
        .parse(&buffer[..offset])
        .map_err(|_| Error::Generic("header parsing error"))?
        .is_complete()
    {
        return Err(Error::Generic("incomplete headers"));
    }

    println!("{} path = {:?}", task_n, req.path);

    match req.path {
        Some("/konkers-music.svg") => {
            send_static_gzip(socket, include_bytes!("html/konkers-music.svg.gz")).await?
        }

        _ => send_static(socket, include_bytes!("html/index.html")).await?,
    }

    socket.flush().await?;
    Ok(())
}
