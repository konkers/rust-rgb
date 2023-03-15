use bitfield_struct::bitfield;
use embassy_net::tcp::{self, TcpSocket};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use embedded_io::asynch::{Read, Write};
use esp32c3_hal::i2c::I2C;
use esp32c3_hal::peripherals::I2C0;
use esp32c3_hal::prelude::*;
use esp_println::println;

use crate::{Error, Result};

const FUSB302_ADDR: u8 = 0x22;

#[repr(u8)]
enum Fusb302Register {
    DeviceId = 0x01,
    Switches0 = 0x02,
    Switches1 = 0x03,
    Measure = 0x04,
    Slice = 0x05,
    Control0 = 0x06,
    Control1 = 0x07,
    Control2 = 0x08,
    Control3 = 0x09,
    Mask1 = 0x0a,
    Power = 0x0b,
    Reset = 0x0c,
    OCPreg = 0x0d,
    MaskA = 0x0e,
    MaskB = 0x0f,
    Control4 = 0x10,
    Status0A = 0x3c,
    Status1A = 0x3d,
    InterruptA = 0x3e,
    InterruptB = 0x3f,
    Status0 = 0x40,
    Status1 = 0x41,
    Interrupt = 0x42,
    Fifos = 0x43,
}

#[bitfield(u8)]
struct DeviceId {
    #[bits(2)]
    revision: u8,
    #[bits(2)]
    product: u8,
    #[bits(4)]
    version: u8,
}

#[bitfield(u8)]
struct Switches0 {
    pdwn1: bool,
    pdwn2: bool,
    meas_cc1: bool,
    meas_cc2: bool,
    vconn_cc1: bool,
    vconn_cc2: bool,
    pu_en1: bool,
    pu_en2: bool,
}

#[bitfield(u8)]
struct Switches1 {
    txcc1: bool,
    txcc2: bool,
    auto_crc: bool,
    _reserved: bool,
    data_role: bool,
    #[bits(2)]
    spec_rev: u8,
    power_role: bool,
}

#[bitfield(u8)]
struct Measure {
    mdac0: bool,
    mdac1: bool,
    mdac2: bool,
    mdac3: bool,
    mdac4: bool,
    mdac5: bool,
    meas_vbus: bool,
    _reserved: bool,
}

#[bitfield(u8)]
struct Slice {
    sdac0: bool,
    sdac1: bool,
    sdac2: bool,
    sdac3: bool,
    sdac4: bool,
    sdac5: bool,
    sdac_hys2: bool,
    sdac_hys1: bool,
}

#[bitfield(u8)]
struct Control0 {
    tx_start: bool,
    auto_pre: bool,
    #[bits(2)]
    host_cur: u8,
    _reserved0: bool,
    int_mask: bool,
    tx_flush: bool,
    _reserved1: bool,
}

#[bitfield(u8)]
struct Control1 {
    ensop1: bool,
    ensop2: bool,
    rx_flush: bool,
    _reserved0: bool,
    bist_mode2: bool,
    ensop1db: bool,
    ensop2db: bool,
    _reserved1: bool,
}

#[bitfield(u8)]
struct Control2 {
    toggle: bool,
    #[bits(2)]
    mode: u8,
    wake_en: bool,
    _reserved0: bool,
    tog_rd_only: bool,
    tog_save_pwr1: bool,
    tog_save_pwr2: bool,
}

#[bitfield(u8)]
struct Control3 {
    auto_retry: bool,
    #[bits(2)]
    n_retries: u8,
    auto_hardreset: bool,
    auto_softreset: bool,
    bist_t_mode: bool,
    send_hard_reset: bool,
    _reserved: bool,
}

#[bitfield(u8)]
struct Mask1 {
    m_bc_lvl: bool,
    m_collision: bool,
    m_wake: bool,
    m_alert: bool,
    m_crc_chk: bool,
    m_comp_chng: bool,
    m_activity: bool,
    m_vbusok: bool,
}

#[bitfield(u8)]
struct Power {
    pwr0: bool,
    pwr1: bool,
    pwr2: bool,
    pwr3: bool,
    #[bits(4)]
    _reserved: u8,
}

#[bitfield(u8)]
struct Reset {
    sw_res: bool,
    pd_reset: bool,
    #[bits(6)]
    _reserved: u8,
}

#[bitfield(u8)]
struct OCPreg {
    ocp_cur0: bool,
    ocp_cur1: bool,
    ocp_cur2: bool,
    ocp_cur3: bool,
    #[bits(4)]
    _reserved: u8,
}

#[bitfield(u8)]
struct Maska {
    m_hardrst: bool,
    m_softrst: bool,
    m_txsent: bool,
    m_hardsent: bool,
    m_retryfail: bool,
    m_softfail: bool,
    m_togdone: bool,
    m_ocp_temp: bool,
}

#[bitfield(u8)]
struct Maskb {
    m_gcrcsent: bool,
    #[bits(7)]
    _reserved: u8,
}

#[bitfield(u8)]
struct Control4 {
    tog_exit_aud: bool,
    #[bits(7)]
    _reserved: u8,
}

#[bitfield(u8)]
struct Status0a {
    hardrst: bool,
    softrst: bool,
    power2: bool,
    power3: bool,
    retryfail: bool,
    softfail: bool,
    #[bits(2)]
    _reserved: u8,
}

#[bitfield(u8)]
struct Status1a {
    rxsop: bool,
    rxsop1db: bool,
    rssop2db: bool,
    togss1: bool,
    togss2: bool,
    togss3: bool,
    #[bits(2)]
    _reserved: u8,
}

#[bitfield(u8)]
struct Interrupta {
    i_hardrst: bool,
    i_softrst: bool,
    i_txsent: bool,
    i_hardsent: bool,
    i_retryfail: bool,
    i_softfail: bool,
    i_togdone: bool,
    i_ocp_temp: bool,
}

#[bitfield(u8)]
struct Interruptb {
    i_gcrcs_ent: bool,
    #[bits(7)]
    _reserved: u8,
}

#[bitfield(u8)]
struct Status0 {
    #[bits(2)]
    bc_lvl: u8,
    wake: bool,
    alert: bool,
    crc_chk: bool,
    comp: bool,
    activity: bool,
    vbusok: bool,
}

#[bitfield(u8)]
struct Status1 {
    ocp: bool,
    overtemp: bool,
    tx_full: bool,
    tx_empty: bool,
    rx_full: bool,
    rx_empty: bool,
    rxsop1: bool,
    rxsop2: bool,
}

#[bitfield(u8)]
struct Interrupt {
    i_bc_lvl: bool,
    i_collision: bool,
    i_wake: bool,
    i_alert: bool,
    i_crc_chk: bool,
    i_comp_chng: bool,
    i_activity: bool,
    i_vbusok: bool,
}

async fn i2c_read_u8(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    device: u8,
    reg: u8,
) -> Result<u8> {
    let mut buffer = [0u8];
    let mut i2c = i2c.lock().await;
    i2c.write_read(device, &[reg], &mut buffer)?;

    Ok(buffer[0])
}

async fn i2c_write_u8(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    device: u8,
    reg: u8,
    data: u8,
) -> Result<()> {
    let mut i2c = i2c.lock().await;
    i2c.write(device, &[reg, data])?;
    Ok(())
}

async fn fusb302_read_u8(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    register: Fusb302Register,
) -> Result<u8> {
    i2c_read_u8(i2c, FUSB302_ADDR, register as u8).await
}

async fn fusb302_write_u8(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    register: Fusb302Register,
    data: u8,
) -> Result<()> {
    i2c_write_u8(i2c, FUSB302_ADDR, register as u8, data).await
}

macro_rules! fusb302_read_reg {
    ($i2c:expr, $reg:ident) => {
        $reg::from(fusb302_read_u8($i2c, Fusb302Register::$reg).await?)
    };
}

macro_rules! fusb302_write_reg {
    ($i2c:expr, $reg:ident, $data:expr) => {
        fusb302_write_u8($i2c, Fusb302Register::$reg, $data.into()).await?
    };
}

enum PdState {
    Reset,
    PollCC,
    WaitForCapabilities,
}

struct Pd {
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'static, I2C0>>,
    state: PdState,
}

impl Pd {
    fn new(i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'static, I2C0>>) -> Self {
        Self {
            i2c,
            state: PdState::Reset,
        }
    }

    async fn tick(&mut self) -> Result<()> {
        match self.state {
            PdState::Reset => self.handle_reset_state().await,
            PdState::PollCC => self.handle_poll_cc_state().await,
            PdState::WaitForCapabilities => self.handle_wait_for_capabilities_state().await,
        }
    }
    async fn handle_reset_state(&mut self) -> Result<()> {
        // Software reset the chip.
        fusb302_write_reg!(self.i2c, Reset, Reset::new().with_sw_res(true));

        // Power up entire chip.
        fusb302_write_reg!(
            self.i2c,
            Power,
            Power::new()
                .with_pwr0(true)
                .with_pwr1(true)
                .with_pwr2(true)
                .with_pwr3(true)
        );

        // Unmask interrupts.
        fusb302_write_reg!(
            self.i2c,
            Control0,
            Control0::new().with_int_mask(false).with_host_cur(0)
        );

        // Enable packet retries
        fusb302_write_reg!(
            self.i2c,
            Control3,
            Control3::new().with_auto_retry(true).with_n_retries(3)
        );

        self.state = PdState::PollCC;
        Ok(())
    }

    async fn handle_poll_cc_state(&mut self) -> Result<()> {
        Timer::after(Duration::from_millis(500)).await;

        // sample CC1
        fusb302_write_reg!(
            self.i2c,
            Switches0,
            Switches0::new()
                .with_pdwn1(true)
                .with_pdwn2(true)
                .with_meas_cc1(true)
        );
        Timer::after(Duration::from_millis(20)).await;
        let cc1_val = fusb302_read_reg!(self.i2c, Status0).bc_lvl();

        // sample CC2
        fusb302_write_reg!(
            self.i2c,
            Switches0,
            Switches0::new()
                .with_pdwn1(true)
                .with_pdwn2(true)
                .with_meas_cc2(true)
        );
        Timer::after(Duration::from_millis(20)).await;
        let cc2_val = fusb302_read_reg!(self.i2c, Status0).bc_lvl();

        if cc1_val == cc2_val {
            println!("no cc connected");
            return Ok(());
        }

        let switches1 = Switches1::new().with_spec_rev(1); // 1 == Revision 2.0
        let switches1 = if cc1_val > cc2_val {
            switches1.with_txcc1(true)
        } else {
            switches1.with_txcc2(true)
        };
        fusb302_write_reg!(self.i2c, Switches1, switches1);
        self.state = PdState::WaitForCapabilities;

        Ok(())
    }

    async fn handle_wait_for_capabilities_state(&mut self) -> Result<()> {
        Timer::after(Duration::from_millis(500)).await;
        Ok(())
    }
}

#[embassy_executor::task]
pub(crate) async fn task(i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>) {
    let mut pd = Pd::new(i2c);
    loop {
        if let Err(e) = pd.tick().await {
            println!("pd_error: {e:?}");
        }
    }
}
