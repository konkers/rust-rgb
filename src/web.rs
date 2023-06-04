use embassy_net::tcp::TcpSocket;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embedded_hal_async::i2c::I2c;
use embedded_io::asynch::Write;
use esp32c3_hal::i2c::I2C;
use esp32c3_hal::peripherals::I2C0;
use esp_println::println;

use crate::{Error, Result};

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

async fn i2c_read<I2C, E>(
    socket: &mut TcpSocket<'_>,
    i2c: &Mutex<NoopRawMutex, &'static mut I2C>,
    dev_addr: u8,
    reg_addr: u8,
) -> Result<()>
where
    I2C: I2c<Error = E>,
    Error: From<E>,
{
    let mut i2c = i2c.lock().await;
    let mut buffer = [0u8];
    println!("reading {reg_addr:x} from {dev_addr:x}");
    i2c.write_read(dev_addr, &[reg_addr], &mut buffer).await?;
    println!("{buffer:x?}");
    socket
        .write_all(b"HTTP/1.0 200 OK\r\n\r\nlook at console")
        .await?;
    Ok(())
}

async fn i2c_read_multiple<I2C, E>(
    socket: &mut TcpSocket<'_>,
    i2c: &Mutex<NoopRawMutex, &'static mut I2C>,
    dev_addr: u8,
    reg_addr: u8,
    len: usize,
) -> Result<()>
where
    I2C: I2c<Error = E>,
    Error: From<E>,
{
    if len > 16 {
        return Err(Error::Generic("can't read more than 16 bytes at once"));
    }

    let mut i2c = i2c.lock().await;
    let mut buffer = [0u8; 16];
    println!("reading {len} bytes from {reg_addr:x} from {dev_addr:x}");
    i2c.write_read(dev_addr, &[reg_addr], &mut buffer[..len])
        .await?;
    println!("{buffer:x?}");
    socket
        .write_all(b"HTTP/1.0 200 OK\r\n\r\nlook at console")
        .await?;
    Ok(())
}

async fn i2c_write<I2C, E>(
    socket: &mut TcpSocket<'_>,
    i2c: &Mutex<NoopRawMutex, &'static mut I2C>,
    dev_addr: u8,
    reg_addr: u8,
    data: u8,
) -> Result<()>
where
    I2C: I2c<Error = E>,
    Error: From<E>,
{
    let mut i2c = i2c.lock().await;
    println!("writing {data:x} to {reg_addr:x} from {dev_addr:x}");
    i2c.write(dev_addr, &[reg_addr, data]).await?;
    socket
        .write_all(b"HTTP/1.0 200 OK\r\n\r\nlook at console")
        .await?;
    Ok(())
}

pub async fn handle_connection(
    task_n: u32,
    socket: &mut TcpSocket<'_>,
    i2c: &Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
) -> Result<()> {
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

    if let Some(path) = req.path {
        if path.starts_with("/i2c/read/") {
            let mut parts_iter = path.split("/");
            let dev_addr_str = parts_iter
                .nth(3)
                .ok_or_else(|| Error::Generic("Can't find dev_addr"))?;
            let reg_addr_str = parts_iter
                .nth(0)
                .ok_or_else(|| Error::Generic("Can't find reg_addr"))?;
            let dev_addr = u8::from_str_radix(dev_addr_str, 16)
                .map_err(|_| Error::Generic("Can't parse dev_addr"))?;
            let reg_addr = u8::from_str_radix(reg_addr_str, 16)
                .map_err(|_| Error::Generic("Can't parse reg_addr"))?;
            println!("{dev_addr:x} {reg_addr:x}");
            i2c_read(socket, i2c, dev_addr, reg_addr).await?;
        } else if path.starts_with("/i2c/read_n/") {
            let mut parts_iter = path.split("/");
            let dev_addr_str = parts_iter
                .nth(3)
                .ok_or_else(|| Error::Generic("Can't find dev_addr"))?;
            let reg_addr_str = parts_iter
                .nth(0)
                .ok_or_else(|| Error::Generic("Can't find reg_addr"))?;
            let len_str = parts_iter
                .nth(0)
                .ok_or_else(|| Error::Generic("Can't find len"))?;
            let dev_addr = u8::from_str_radix(dev_addr_str, 16)
                .map_err(|_| Error::Generic("Can't parse dev_addr"))?;
            let reg_addr = u8::from_str_radix(reg_addr_str, 16)
                .map_err(|_| Error::Generic("Can't parse reg_addr"))?;
            let len =
                u8::from_str_radix(len_str, 16).map_err(|_| Error::Generic("Can't parse data"))?;

            println!("{dev_addr:x} {reg_addr:x} {len:x}");
            i2c_read_multiple(socket, i2c, dev_addr, reg_addr, len as usize).await?;
        } else if path.starts_with("/i2c/write/") {
            let mut parts_iter = path.split("/");
            let dev_addr_str = parts_iter
                .nth(3)
                .ok_or_else(|| Error::Generic("Can't find dev_addr"))?;
            let reg_addr_str = parts_iter
                .nth(0)
                .ok_or_else(|| Error::Generic("Can't find reg_addr"))?;
            let data_str = parts_iter
                .nth(0)
                .ok_or_else(|| Error::Generic("Can't find data"))?;
            let dev_addr = u8::from_str_radix(dev_addr_str, 16)
                .map_err(|_| Error::Generic("Can't parse dev_addr"))?;
            let reg_addr = u8::from_str_radix(reg_addr_str, 16)
                .map_err(|_| Error::Generic("Can't parse reg_addr"))?;
            let data =
                u8::from_str_radix(data_str, 16).map_err(|_| Error::Generic("Can't parse data"))?;

            println!("{dev_addr:x} {reg_addr:x} {data:x}");
            i2c_write(socket, i2c, dev_addr, reg_addr, data).await?;
        } else {
            match path {
                "/konkers-music.svg" => {
                    send_static_gzip(socket, include_bytes!("html/konkers-music.svg.gz")).await?
                }

                _ => send_static(socket, include_bytes!("html/index.html")).await?,
            }
        }
    }

    socket.flush().await?;
    Ok(())
}
