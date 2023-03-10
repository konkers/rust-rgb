use core::array::TryFromSliceError;
use core::cmp::{max, min};

use byteorder::LittleEndian;
use embassy_net::{udp, IpAddress, Ipv4Address};
use embassy_net::{
    udp::UdpSocket, Config, IpListenEndpoint, PacketMetadata, Stack, StackResources,
};
use embassy_time::{Duration, Timer};
use embedded_hal_async::spi::SpiBusWrite;
use esp32c3_hal::gpio::{
    Bank0GpioRegisterAccess, Gpio2Signals, GpioPin, InputOutputAnalogPinType,
    SingleCoreInteruptStatusRegisterAccessBank0,
};
use esp32c3_hal::pulse_control::{Channel0, ConfiguredChannel0};
use esp32c3_hal::utils::SmartLedsAdapter;
use esp32c3_hal::PulseControl;
use esp_println::println;
use esp_wifi::wifi::{WifiController, WifiDevice, WifiEvent, WifiState};
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};
use smart_leds::{brightness, gamma, SmartLedsWrite, RGB};
use smoltcp::wire::IpEndpoint;

use crate::buffer::{self, MutBuffer, OldBuffer};
use crate::ws2812::{self, Ws2812};

#[derive(Debug)]
pub enum Error {
    HeaderMissing,
    Buffer(buffer::Error),
    FromSlice(TryFromSliceError),
    UdpError(udp::Error),
    Unimplemented,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::HeaderMissing => write!(f, "Artnet header missing"),
            Error::Buffer(e) => write!(f, "Buffer error {e}"),
            Error::FromSlice(e) => write!(f, "From slice error {e}"),
            Error::UdpError(e) => write!(f, "Udp error {e:?}"),
            Error::Unimplemented => write!(f, "Unimplemented"),
        }
    }
}

impl core::error::Error for Error {}

impl From<buffer::Error> for Error {
    fn from(value: buffer::Error) -> Self {
        Self::Buffer(value)
    }
}

impl From<TryFromSliceError> for Error {
    fn from(value: TryFromSliceError) -> Self {
        Self::FromSlice(value)
    }
}

impl From<udp::Error> for Error {
    fn from(value: udp::Error) -> Self {
        Self::UdpError(value)
    }
}

type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, FromPrimitive, ToPrimitive)]
#[repr(u16)]
pub enum Opcode {
    Poll = 0x2000,
    PollReply = 0x2100,
    DiagData = 0x2300,
    Command = 0x2400,
    Output = 0x5000,
    Nzs = 0x5100,
    Sync = 0x5200,
    Address = 0x6000,
    Input = 0x7000,
    TodRequest = 0x8000,
    TodData = 0x8100,
    TodControl = 0x8200,
    Rdm = 0x8300,
    RdmSub = 0x8400,
    VideoSetup = 0xa010,
    VideoPalette = 0xa020,
    VideoData = 0xa040,
    Firmware = 0xf200,
    FirmwareReply = 0xf300,
    FileTn = 0xf400,
    FileFn = 0xf500,
    FileFnReply = 0xf600,
    IpProg = 0xf800,
    IpProgReply = 0xf900,
    Media = 0x9000,
    MediaPatch = 0x9100,
    MediaControl = 0x9200,
    MediaControlReply = 0x9300,
    TimeCode = 0x9700,
    TimeSync = 0x9800,
    Trigger = 0x9900,
    Directory = 0x9a00,
    DirectoryReply = 0x9b00,
}

#[derive(Clone, Copy, Debug, FromPrimitive, ToPrimitive)]
#[repr(u16)]
pub enum NodeRepotCode {
    Debug = 0x0000,
    PowerOk = 0x0001,
    PowerFail = 0x0002,
    SocketWr1 = 0x0003,
    ParseFail = 0x0004,
    UdpFail = 0x0005,
    ShNameOk = 0x0006,
    LoNameOk = 0x0007,
    DmxError = 0x0008,
    DmxUdpFull = 0x0009,
    DmxRxFull = 0x000a,
    SwitchErr = 0x000b,
    ConfigErr = 0x000c,
    DmxShort = 0x000d,
    FirmwareFail = 0x000e,
    UserFail = 0x000f,
    FactoryRes = 0x0010,
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum StyleCode {
    Node = 0x00,
    Controller = 0x01,
    Media = 0x02,
    Route = 0x03,
    Backup = 0x04,
    Config = 0x05,
    Visual = 0x06,
}

const ARTNET_ID: &'static [u8; 8] = b"Art-Net\0";

#[derive(Debug)]
pub struct Poll {
    pub prot_ver: [u8; 2],
    pub flags: u8,
    pub diag_priority: u8,
    pub target_port_addr_top: Option<u16>,
    pub target_port_addr_bot: Option<u16>,
}

impl Poll {
    fn parse(buf: &mut OldBuffer<LittleEndian>) -> Result<Self> {
        let mut prot_ver = [0u8; 2];
        buf.read_buf(&mut prot_ver)?;
        let flags = buf.read_u8()?;
        let diag_priority = buf.read_u8()?;

        // TODO: semanic flags
        let (target_port_addr_top, target_port_addr_bot) = if (flags & (0x1 << 5)) != 0 {
            (Some(buf.read_u16()?), Some(buf.read_u16()?))
        } else {
            (None, None)
        };

        Ok(Poll {
            prot_ver,
            flags,
            diag_priority,
            target_port_addr_top,
            target_port_addr_bot,
        })
    }
}

#[derive(Debug)]
pub struct PollReply {
    pub ip_address: [u8; 4],
    pub port: u16,
    pub vers_info: [u8; 2],
    pub net_switch: u8,
    pub sub_switch: u8,
    pub oem: [u8; 2],
    pub ubea_version: u8,
    pub status_1: u8,
    pub esta_man: [u8; 2],
    pub short_name: [u8; 18],
    pub long_name: [u8; 64],
    pub node_report: [u8; 64],
    pub num_ports: [u8; 2],
    pub port_types: [u8; 4],
    pub good_input: [u8; 4],
    pub good_output: [u8; 4],
    pub sw_in: [u8; 4],
    pub sw_out: [u8; 4],
    pub acn_priority: u8,
    pub sw_macro: u8,
    pub sw_remote: u8,
    pub spare: [u8; 3],
    pub style: u8,
    pub mac: [u8; 6],
    pub bind_ip: [u8; 4],
    pub bind_index: u8,
    pub status_2: u8,
    pub good_output_b: [u8; 4],
    pub status_3: u8,
    pub default_resp_uid: [u8; 6],
    // padding: [u8; 15],
}

impl PollReply {
    fn parse(buf: &mut OldBuffer<LittleEndian>) -> Result<Self> {
        Ok(Self {
            ip_address: buf.read()?,
            port: buf.read_u16()?,
            vers_info: buf.read()?,
            net_switch: buf.read_u8()?,
            sub_switch: buf.read_u8()?,
            oem: buf.read()?,
            ubea_version: buf.read_u8()?,
            status_1: buf.read_u8()?,
            esta_man: buf.read()?,
            short_name: buf.read()?,
            long_name: buf.read()?,
            node_report: buf.read()?,
            num_ports: buf.read()?,
            port_types: buf.read()?,
            good_input: buf.read()?,
            good_output: buf.read()?,
            sw_in: buf.read()?,
            sw_out: buf.read()?,
            acn_priority: buf.read_u8()?,
            sw_macro: buf.read_u8()?,
            sw_remote: buf.read_u8()?,
            spare: buf.read()?,
            style: buf.read_u8()?,
            mac: buf.read()?,
            bind_ip: buf.read()?,
            bind_index: buf.read_u8()?,
            status_2: buf.read_u8()?,
            good_output_b: buf.read()?,
            status_3: buf.read_u8()?,
            default_resp_uid: buf.read()?,
            // We ignore the padding at the end.  Should we check it?
        })
    }

    fn write(&self, buf: &mut MutBuffer<LittleEndian>) -> Result<()> {
        buf.write_u16(Opcode::PollReply.to_u16().unwrap())?;
        buf.write(&self.ip_address)?;
        buf.write_u16(self.port)?;
        buf.write(&self.vers_info)?;
        buf.write_u8(self.net_switch)?;
        buf.write_u8(self.sub_switch)?;
        buf.write(&self.oem)?;
        buf.write_u8(self.ubea_version)?;
        buf.write_u8(self.status_1)?;
        buf.write(&self.esta_man)?;
        buf.write(&self.short_name)?;
        buf.write(&self.long_name)?;
        buf.write(&self.node_report)?;
        buf.write(&self.num_ports)?;
        buf.write(&self.port_types)?;
        buf.write(&self.good_input)?;
        buf.write(&self.good_output)?;
        buf.write(&self.sw_in)?;
        buf.write(&self.sw_out)?;
        buf.write_u8(self.acn_priority)?;
        buf.write_u8(self.sw_macro)?;
        buf.write_u8(self.sw_remote)?;
        buf.write(&self.spare)?;
        buf.write_u8(self.style)?;
        buf.write(&self.mac)?;
        buf.write(&self.bind_ip)?;
        buf.write_u8(self.bind_index)?;
        buf.write_u8(self.status_2)?;
        buf.write(&self.good_output_b)?;
        buf.write_u8(self.status_3)?;
        buf.write(&self.default_resp_uid)?;
        buf.write(&[0u8; 15])?; // Padding

        Ok(())
    }
}

#[derive(Debug)]
pub struct Output<'a> {
    prot_ver: [u8; 2],
    sequence: u8,
    physical: u8,
    sub_uni: u8,
    net: u8,
    data: &'a [u8],
}
impl<'a> Output<'a> {
    fn parse(buf: &mut OldBuffer<'a, LittleEndian>) -> Result<Self> {
        let prot_ver = buf.read()?;
        let sequence = buf.read_u8()?;
        let physical = buf.read_u8()?;
        let sub_uni = buf.read_u8()?;
        let net = buf.read_u8()?;
        let len_raw: [u8; 2] = buf.read()?;
        let len = (len_raw[0] as u16) << 8 | len_raw[1] as u16;
        let data = buf.take(len as usize)?;
        Ok(Self {
            prot_ver,
            sequence,
            physical,
            sub_uni,
            net,
            data,
        })
    }
}

#[derive(Debug)]
pub struct Unknown<'a> {
    pub data: &'a [u8],
}

#[derive(Debug)]
pub enum Packet<'a> {
    Poll(Poll),
    PollReply(PollReply),
    Output(Output<'a>),
    Unknown(Unknown<'a>),
}

impl<'a> Packet<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Packet<'a>> {
        let buf = &mut OldBuffer::<LittleEndian>::new(data);
        let header = buf.take(8)?;
        if header != ARTNET_ID {
            return Err(Error::HeaderMissing);
        }

        let opcode = buf.read_u16()?;

        let Some(opcode) = Opcode::from_u16(opcode) else {
        	return Ok(Packet::Unknown(Unknown { data }));
	};

        match opcode {
            Opcode::Poll => Ok(Packet::Poll(Poll::parse(buf)?)),
            Opcode::PollReply => Ok(Packet::PollReply(PollReply::parse(buf)?)),
            Opcode::Output => Ok(Packet::Output(Output::parse(buf)?)),
            _ => Ok(Packet::Unknown(Unknown { data })),
        }
    }

    pub fn write(&self, data: &mut [u8]) -> Result<usize> {
        let buf = &mut MutBuffer::<LittleEndian>::new(data);

        buf.write(ARTNET_ID)?;
        match self {
            Self::PollReply(reply) => reply.write(buf)?,
            _ => return Err(Error::Unimplemented),
        }

        Ok(buf.pos())
    }
}

fn padded_byte_str<const N: usize>(data: &[u8]) -> [u8; N] {
    let mut output = [0u8; N];
    let copy_len = min(data.len(), N);
    output[..copy_len].copy_from_slice(&data[..copy_len]);
    output
}

async fn send_poll_reply(
    socket: &mut UdpSocket<'_>,
    my_address: &Ipv4Address,
    ep: &IpEndpoint,
    buf: &mut [u8],
) -> Result<()> {
    let reply = Packet::PollReply(PollReply {
        ip_address: my_address.as_bytes().try_into()?,
        port: 0x1936,
        vers_info: [0x0, 0x0],
        net_switch: 0,
        sub_switch: 0,
        oem: [0x00, 0xff],
        ubea_version: 0,
        status_1: 0xe0,
        esta_man: [0xff, 0xff],
        short_name: padded_byte_str(b"Blinky"),
        long_name: padded_byte_str(b"Konkers' Blinky Toy"),
        node_report: padded_byte_str(b"It's all good!"),
        num_ports: [0, 1],
        port_types: [0xc0, 0x00, 0x00, 0x00],
        good_input: [8; 4],
        good_output: [0x82, 0, 0, 0],
        sw_in: [0, 0, 0, 0],
        sw_out: [0, 0, 0, 0],
        acn_priority: 0,
        sw_macro: 0,
        sw_remote: 0,
        spare: [0; 3],
        style: 0,
        mac: [0x34, 0x85, 0x18, 0x00, 0xc5, 0xd0], // TODO: get from stack
        bind_ip: my_address.as_bytes().try_into()?,
        bind_index: 1,
        status_2: 0x1e,
        good_output_b: [0xc0; 4],
        status_3: 0x30,
        default_resp_uid: [0; 6], //[0x6a, 0x6b, 0xee, 0x22, 0x17, 0x43],
    });

    let len = reply.write(buf)?;
    socket
        .send_to(
            &buf[..len],
            IpEndpoint {
                addr: IpAddress::Ipv4(Ipv4Address([0xff, 0xff, 0xff, 0xff])),
                port: 6454,
            },
        )
        .await?;
    Ok(())
}

#[embassy_executor::task]
pub(crate) async fn task(
    stack: &'static Stack<WifiDevice>,
    spi: &'static mut crate::SpiType<'static>,
) {
    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut rx_buffer = [0; 4096];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_buffer = [0; 4096];
    let mut buf = [0; 4096];

    const NUM_LEDS: usize = 120;
    const LED_BUF_LEN: usize = ws2812::buffer_len(NUM_LEDS);
    let mut led_buf = [0u8; LED_BUF_LEN];

    let my_address = loop {
        if let Some(config) = stack.config() {
            break config.address.address();
        }
        Timer::after(Duration::from_millis(500)).await;
    };

    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    socket.bind(6454).unwrap();
    loop {
        let (length, ep) = socket.recv_from(&mut buf).await.unwrap();
        if let Ok(packet) = Packet::parse(&buf[..length]) {
            match packet {
                Packet::Poll(poll) => {
                    //println!("sending poll reply to {poll:x?}");
                    //Timer::after(Duration::from_millis(150)).await;
                    send_poll_reply(&mut socket, &my_address, &ep, &mut buf)
                        .await
                        .ok();
                }
                Packet::Output(output) => {
                    //println!("got output packet: {output:x?}");
                    if output.sub_uni == 0 {
                        // let brightness = output.data[9 + 6] as u16;
                        // let r = output.data[9] as u16 * brightness / 256;
                        // let g = output.data[10] as u16 * brightness / 256;
                        // let b = output.data[11] as u16 * brightness / 256;
                        let mut ws = Ws2812::<LED_BUF_LEN>::new(&mut led_buf);
                        for i in (0..NUM_LEDS) {
                            let base = 32 + i / (NUM_LEDS / 10) * 3;
                            let r = output.data[base + 0]; // as u16 * brightness / 256;
                            let g = output.data[base + 1]; // as u16 * brightness / 256;
                            let b = output.data[base + 2]; // as u16 * brightness / 256;

                            ws.set_led(i, r, g, b);
                        }
                        let led_buf = ws.into_buf();

                        let _ret = spi.write(&led_buf).await;
                    }
                }
                _ => (), //println!("artnet packet: {:x?}", &packet);
            }
        } else {
            //println!("artnet {:x?}", &buf[..length]);
        }
    }
}
