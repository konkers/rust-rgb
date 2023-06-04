use embassy_futures::join::join;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use embedded_hal_async::digital::Wait;
use esp32c3_hal::gpio::{
    Bank0GpioRegisterAccess, Floating, Gpio7Signals, GpioPin, Input, InputOutputPinType,
    SingleCoreInteruptStatusRegisterAccessBank0,
};
use esp32c3_hal::i2c::I2C;
use esp32c3_hal::peripherals::I2C0;
use esp_println::println;
use num_traits::FromPrimitive;

mod bq25620;
mod fusb302;
mod i2c;
mod proto;

use crate::{fusb302_read_reg, fusb302_write_reg};
use crate::{Error, Result};
use fusb302::{
    fusb302_read_fifo, fusb302_read_fifo_u16, fusb302_read_fifo_u32, fusb302_read_fifo_u8,
    fusb302_read_status, fusb302_read_u8, Control0, Control1, Control2, Control3, DeviceId,
    Fusb302MessageBuffer, Fusb302Register, Mask1, MaskA, MaskB, Measure, Power, Reset, RxToken,
    RxTokenType, Status, Status0, Status1, Switches0, Switches1,
};
use proto::*;

use self::bq25620::Bq25620;

// Data messges have a max of 7 * 32 bit objects.
const MAX_PAYLOAD_SIZE: usize = 7 * 4;

enum PdState {
    Reset,
    WaitForVbus,
    PollCC,
    Online,
}

struct Pd {
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'static, I2C0>>,
    pd_int_n: GpioPin<
        Input<Floating>,
        Bank0GpioRegisterAccess,
        SingleCoreInteruptStatusRegisterAccessBank0,
        InputOutputPinType,
        Gpio7Signals,
        7,
    >,
    state: PdState,
    status: Status,
    pdos: [FixedSupplyPdo; 7],
    num_pdos: usize,
    message_id: u8,
}

impl Pd {
    fn new(
        i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'static, I2C0>>,
        pd_int_n: GpioPin<
            Input<Floating>,
            Bank0GpioRegisterAccess,
            SingleCoreInteruptStatusRegisterAccessBank0,
            InputOutputPinType,
            Gpio7Signals,
            7,
        >,
    ) -> Self {
        Self {
            i2c,
            pd_int_n,
            state: PdState::Reset,
            status: Default::default(),
            pdos: [FixedSupplyPdo::new(); 7],
            num_pdos: 0,
            message_id: 0,
        }
    }

    async fn flush_rx_fifo(&mut self) -> Result<()> {
        fusb302_write_reg!(self.i2c, Control1, Control1::new().with_rx_flush(true))
    }

    async fn fusb_reset(&mut self) -> Result<()> {
        // flush tx buffer
        fusb302_write_reg!(
            self.i2c,
            Control0,
            Control0::new().with_host_cur(1).with_tx_flush(true)
        )?;

        self.flush_rx_fifo().await?;

        fusb302_write_reg!(self.i2c, Reset, Reset::new().with_pd_reset(true))?;

        Ok(())
    }

    async fn fusb_read_id(&mut self) -> Result<DeviceId> {
        let val = fusb302_read_u8(self.i2c, Fusb302Register::DeviceId).await?;
        if val == 0 || val == 0xff {
            return Err(Error::InvalidDeviceId);
        }

        Ok(DeviceId::from(val))
    }

    async fn fusb_setup(&mut self) -> Result<()> {
        // Software reset the chip.
        fusb302_write_reg!(self.i2c, Reset, Reset::new().with_sw_res(true))?;

        // Wait till the chip responds with its ID.
        let mut retries = 5;
        loop {
            if self.fusb_read_id().await.is_ok() {
                break;
            }
            retries -= 1;
            if retries == 0 {
                return Err(Error::SoftResetFailure);
            }
        }

        // Power up entire chip.
        fusb302_write_reg!(
            self.i2c,
            Power,
            Power::new()
                .with_pwr0(true)
                .with_pwr1(true)
                .with_pwr2(true)
                .with_pwr3(true)
        )?;

        // Unmask interrupts.
        fusb302_write_reg!(self.i2c, Mask1, Mask1::new())?;
        fusb302_write_reg!(self.i2c, MaskA, MaskA::new())?;
        fusb302_write_reg!(self.i2c, MaskB, MaskB::new())?;
        fusb302_write_reg!(self.i2c, Control0, Control0::new().with_host_cur(3))?;

        // Enable packet retries
        fusb302_write_reg!(
            self.i2c,
            Control3,
            Control3::new().with_auto_retry(true).with_n_retries(3)
        )?;

        // Set defaults for Control 2
        fusb302_write_reg!(self.i2c, Control2, Control2::new())?;

        self.flush_rx_fifo().await?;

        Ok(())
    }

    async fn detect_cc_line(&mut self) -> Result<()> {
        // Reset Measure register to default values
        fusb302_write_reg!(self.i2c, Measure, Measure::new().with_mdac(0b11_0001))?;

        // sample CC1
        fusb302_write_reg!(
            self.i2c,
            Switches0,
            Switches0::new()
                .with_pdwn1(true)
                .with_pdwn2(true)
                .with_meas_cc1(true)
        )?;
        Timer::after(Duration::from_millis(20)).await; // TODO: replace with poll of status bit
        let cc1_val = fusb302_read_reg!(self.i2c, Status0)?.bc_lvl();

        // sample CC2
        fusb302_write_reg!(
            self.i2c,
            Switches0,
            Switches0::new()
                .with_pdwn1(true)
                .with_pdwn2(true)
                .with_meas_cc2(true)
        )?;
        Timer::after(Duration::from_millis(20)).await; // TODO: replace with poll of status bit
        let cc2_val = fusb302_read_reg!(self.i2c, Status0)?.bc_lvl();

        if cc1_val == cc2_val {
            return Err(Error::NoCcDetected);
        }

        let use_cc1 = cc1_val > cc2_val;
        let use_cc2 = cc2_val > cc1_val;

        fusb302_write_reg!(
            self.i2c,
            Switches0,
            Switches0::new()
                .with_pdwn1(true)
                .with_pdwn2(true)
                .with_meas_cc1(use_cc1)
                .with_meas_cc2(use_cc2)
        )?;

        self.flush_rx_fifo().await?;

        // Enableing AutoCRC means that the FUSB302 will auto ACK packets
        // from our peer.  If we don't respond the messages in time, the
        // peer will likely disconnect.
        fusb302_write_reg!(
            self.i2c,
            Switches1,
            Switches1::new()
                .with_txcc1(use_cc1)
                .with_txcc2(use_cc2)
                .with_auto_crc(true)
                .with_spec_rev(0) // 0 == Revision 1.0
        )?;

        Ok(())
    }

    async fn poll_status(&mut self) -> Result<()> {
        self.status = fusb302_read_status(self.i2c).await?;
        //println!("{:?}", status);

        if self.status.interrupt_a.i_txsent() {
            self.handle_tx_sent().await?;
        }

        if self.status.interrupt_a.i_retryfail() {
            self.handle_retry_fail().await?;
        }

        if self.status.interrupt_a.i_ocp_temp() || self.status.status_1.overtemp() {
            self.handle_over_temp().await?;
        }

        if self.status.interrupt_b.i_gcrcsent() {
            self.handle_new_data().await?;
        }

        Ok(())
    }

    async fn wait_for_interrupt(&mut self) -> Result<()> {
        // Wait for an interrupt
        self.pd_int_n.wait_for_low().await?;

        self.poll_status().await
    }

    async fn tick(&mut self) -> Result<()> {
        match self.state {
            PdState::Reset => self.handle_reset_state().await,
            PdState::WaitForVbus => self.handle_wait_for_vbus_state().await,
            PdState::PollCC => self.handle_poll_cc_state().await,
            PdState::Online => self.handle_online_state().await,
        }
    }

    async fn handle_reset_state(&mut self) -> Result<()> {
        if self.fusb_setup().await.is_ok() {
            println!("Reset done");
            self.state = PdState::WaitForVbus;
        }
        Ok(())
    }

    async fn handle_wait_for_vbus_state(&mut self) -> Result<()> {
        // Enable pulldowns and start measuring vbus.
        fusb302_write_reg!(
            self.i2c,
            Measure,
            Measure::new().with_meas_vbus(true).with_mdac(0)
        )?;

        fusb302_write_reg!(
            self.i2c,
            Switches0,
            Switches0::new().with_pdwn1(true).with_pdwn2(true)
        )?;

        loop {
            self.poll_status().await?;
            if self.status.status_0.vbusok() {
                break;
            }
            self.wait_for_interrupt().await?;
        }
        println!("vbus detected done");

        self.state = PdState::PollCC;

        Ok(())
    }

    async fn handle_poll_cc_state(&mut self) -> Result<()> {
        Timer::after(Duration::from_millis(500)).await;
        if self.detect_cc_line().await.is_ok() {
            self.fusb_reset().await?;
            self.state = PdState::Online;
        }

        Ok(())
    }

    async fn handle_online_state(&mut self) -> Result<()> {
        self.wait_for_interrupt().await?;

        if !self.status.status_0.vbusok() {
            println!("vbus disconnect");
            self.state = PdState::Reset;
        }

        Ok(())
    }

    async fn handle_tx_sent(&self) -> Result<()> {
        println!("tx sent");
        Ok(())
    }

    async fn handle_retry_fail(&self) -> Result<()> {
        println!("retry fail");
        Ok(())
    }

    async fn handle_over_temp(&self) -> Result<()> {
        println!("over temp");
        Ok(())
    }

    async fn handle_new_data(&mut self) -> Result<()> {
        let mut payload = [0u8; MAX_PAYLOAD_SIZE];

        while !fusb302_read_reg!(self.i2c, Status1)?.rx_empty() {
            let token = RxToken::from(fusb302_read_fifo_u8(self.i2c).await?);
            if token.token() != RxTokenType::Sop {
                // Skip non SOP tokens.
                continue;
            }

            let header = Header::from(fusb302_read_fifo_u16(self.i2c).await?);
            if header.num_data_objects() > 0 {
                fusb302_read_fifo(self.i2c, &mut payload[0..(header.num_data_objects() * 4)])
                    .await?;
            }

            // The FUSB302 has already verified the crc but we still need to
            // clear it from the FIFO.
            let _crc = fusb302_read_fifo_u32(self.i2c).await?;

            self.handle_message(header, &payload[0..(header.num_data_objects() * 4)])
                .await?;
        }

        Ok(())
    }

    async fn handle_message(&mut self, header: Header, payload: &[u8]) -> Result<()> {
        if header.num_data_objects() > 0 {
            let Some(message_type) = DataMessageType::from_u8(header.message_type()) else {
                self.unhandled_message(header, payload);
                return Ok(());
            };
            match message_type {
                DataMessageType::SourceCapabilities => {
                    self.handle_source_capabilities(payload).await?
                }
                _ => self.unhandled_message(header, payload),
            }
        } else {
            self.unhandled_message(header, payload);
        }

        Ok(())
    }

    async fn handle_source_capabilities(&mut self, payload: &[u8]) -> Result<()> {
        (self.num_pdos, _) = payload
            .iter()
            .cloned()
            .array_chunks::<4>()
            .map(|val| FixedSupplyPdo::from(u32::from_le_bytes(val)))
            .fold((0, &mut self.pdos), |(num_pdos, pdos), pdo| {
                pdos[num_pdos] = pdo;
                (num_pdos + 1, pdos)
            });

        // TODO: set spec revision in header.  See https://github.com/Ralim/usb-pd/blob/main/src/policy_engine_states.cpp#L79

        // TODO: callback for selection
        let (selected_pdo, power) = self.pdos[..self.num_pdos].iter().enumerate().fold(
            (0, 0),
            |(selected_pdo, power), (index, pdo)| {
                let pdo_power = pdo.power();
                if (pdo.voltage() <= 18000) && (pdo_power > power) {
                    (index, pdo_power)
                } else {
                    (selected_pdo, power)
                }
            },
        );

        // It is important that we reply quickly otherwise the remote side will possibly give up.
        let mut msg = Fusb302MessageBuffer::new();
        msg.write_header(
            Header::new()
                .with_message_type(DataMessageType::Request as u8)
                .with_spec_revision(2)
                .with_message_id(self.message_id)
                .with_num_data_objects(1)
                .into(),
        );
        self.message_id = (self.message_id + 1) & 0b111;
        msg.write_data(
            FixedVariableSupplyRequest::new()
                .with_min_operating_current(self.pdos[selected_pdo].max_current())
                .with_operating_current(self.pdos[selected_pdo].max_current())
                .with_no_usb_suspend(true)
                .with_object_position((selected_pdo + 1) as u8)
                .into(),
        )?;
        msg.send(self.i2c).await?;

        //TODO: wait for good crc and accept.

        //TODO: remove debugging
        println!("sent {msg:x?}");
        println!("selected_index: {selected_pdo}");
        println!("selected_power: {power}");
        let pdo = &self.pdos[selected_pdo];
        println!("     {pdo:?}");
        println!("     voltage: {} mV", pdo.voltage());
        println!("     max current: {} mA", pdo.max_current());
        // for pdo in &self.pdos[..self.num_pdos] {
        //     println!("     {pdo:?}");
        //     println!("     voltage: {} mV", pdo.voltage());
        //     println!("     max current: {} mA", pdo.max_current());
        //     println!(
        //         "     power: {} W",
        //         pdo.voltage() * pdo.max_current() / (1000 * 1000)
        //     );
        // }
        Ok(())
    }
    fn unhandled_message(&self, header: Header, payload: &[u8]) {
        if false {
            println!("unhandled message:");
            println!("  {header:?}");
            println!("  {payload:x?}");
        }
    }
}

async fn handle_pd(mut pd: Pd) {
    loop {
        if let Err(e) = pd.tick().await {
            println!("pd_error: {e:?}");
        }
    }
}

async fn handle_bq(mut bq: Bq25620) {
    println!("{:?}", bq.init().await);
    loop {
        if let Err(e) = bq.tick().await {
            println!("bq_error: {e:?}");
        }
    }
}

#[embassy_executor::task]
pub(crate) async fn task(
    i2c: &'static Mutex<NoopRawMutex, &'static mut I2C<'_, I2C0>>,
    pd_int_n: GpioPin<
        Input<Floating>,
        Bank0GpioRegisterAccess,
        SingleCoreInteruptStatusRegisterAccessBank0,
        InputOutputPinType,
        Gpio7Signals,
        7,
    >,
) {
    let pd = Pd::new(i2c.clone(), pd_int_n);
    let bq = Bq25620::new(i2c);
    join(handle_pd(pd), handle_bq(bq)).await;
}
