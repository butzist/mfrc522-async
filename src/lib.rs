//! Driver library for interfacing with the MFRC522 contacless communication IC,
//! based on the [embedded-hal](https://docs.rs/embedded-hal-async/latest/embedded_hal_async/) traits.
//!
//! The MFRC522 is a *Proximity Coupling Device* (PCD) and communicates with a
//! *Proximity Integrated Circuit Card* (PICC).
//! The main purpose of the MFRC522 is to give the connected device
//! (where we're running this driver) the ability to read and write data from/to the card.
//!
//! The MFRC522 supports 3 communication interfaces:
//! - SPI
//! - I2C
//! - UART
//!
//! However, currently only SPI communication with additional IRQ line is implemented in this crate.
//!
//! # Quickstart
//! ```rust
//! use mfrc522_async::Mfrc522;
//!
//! // Use your HAL to create an SPI device that implements the embedded-hal-async `SpiDevice` trait.
//! // This device manages the SPI bus and CS pin.
//! let spi = spi::Spi;
//!
//! // Use your HAL to create a input pin that will be used as the IRQ line. It needs to implement
//! the embedded-hal-async `Wait` trait.
//! let irq = digital::Input;
//!
//! let mut mfrc522 = Mfrc522::new(spi, irq).init().unwrap();
//!
//! // The reported version is expected to be 0x91 or 0x92
//! let mfrc522_version = mfrc522.version().unwrap();
//! ```
//!
//! Take a look at [Mfrc522] for information on the functions that are available after initialization.

#![no_std]
#![deny(unsafe_code, missing_docs)]

use embassy_time::{Duration, WithTimeout};
use embedded_hal_async::digital::Wait;
use embedded_hal_async::spi::{Operation, SpiDevice};

mod error;
mod picc;
mod register;
mod util;

pub use error::Error;

pub use picc::{Command, Sak, Type};
pub use register::{
    BUFFER_OVFL, COLL_ERR, CRC_ERR, CRC_IRQ, ERR_IRQ, FLUSH_BUFFER, FORCE_100_ASK, IDLE_IRQ,
    PARITY_ERR, POWER_DOWN, PROTOCOL_ERR, RX_IRQ, Register, RxGain, TEMP_ERR, TIMER_IRQ, WR_ERR,
};
use util::Sealed;

/// Answer To reQuest type A
#[derive(Debug, PartialEq)]
pub struct AtqA {
    bytes: [u8; 2],
}

/// The unique identifier returned by a PICC
pub enum Uid {
    /// Single sized UID, 4 bytes long
    Single(GenericUid<4>),
    /// Double sized UID, 7 bytes long
    Double(GenericUid<7>),
    /// Triple sized UID, 10 bytes long
    Triple(GenericUid<10>),
}

impl Uid {
    /// Get the UID as a byte slice
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Uid::Single(u) => u.as_bytes(),
            Uid::Double(u) => u.as_bytes(),
            Uid::Triple(u) => u.as_bytes(),
        }
    }

    /// Get the type of the PICC that returned the UID
    pub fn get_type(&self) -> Type {
        match self {
            Uid::Single(u) => u.get_type(),
            Uid::Double(u) => u.get_type(),
            Uid::Triple(u) => u.get_type(),
        }
    }
}

/// An identifier that is generic over the size.
pub struct GenericUid<const T: usize>
where
    [u8; T]: Sized,
{
    bytes: [u8; T],
    sak: Sak,
}

impl<const T: usize> GenericUid<T> {
    /// Create a GenericUid from a byte array and a SAK byte.
    pub fn new(bytes: [u8; T], sak_byte: u8) -> Self {
        Self {
            bytes,
            sak: Sak::from(sak_byte),
        }
    }

    /// Get the underlying bytes of the UID
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Get the type of the PICC
    pub fn get_type(&self) -> Type {
        self.sak.get_type()
    }
}

/// Implemented by the different states of the MFRC522 driver.
///
/// This trait cannot be implemented outside of this crate.
pub trait State: Sealed {}

/// The MFRC522 driver starts in this state and needs to be initialized before it can be used.
pub enum Uninitialized {}
/// The MFRC522 driver is ready for use.
pub enum Initialized {}

impl State for Uninitialized {}
impl State for Initialized {}
impl Sealed for Uninitialized {}
impl Sealed for Initialized {}

/// Async MFRC522 driver
pub struct Mfrc522<SPI, IRQ, S: State> {
    spi: SPI,
    irq: IRQ,
    state: core::marker::PhantomData<S>,
}

impl<SPI, IRQ> Mfrc522<SPI, IRQ, Uninitialized>
where
    SPI: SpiDevice,
    IRQ: Wait,
{
    /// Create a new MFRC522 driver from the communication interface.
    pub fn new(spi: SPI, irq: IRQ) -> Self {
        Self {
            spi,
            irq,
            state: core::marker::PhantomData,
        }
    }
}

// The driver can transition to the `Initialized` state using this function
impl<SPI, IRQ> Mfrc522<SPI, IRQ, Initialized>
where
    SPI: SpiDevice,
    IRQ: Wait,
{
    /// Initialize the MFRC522.
    ///
    /// This needs to be called before you can do any other operation.
    pub async fn init(
        mut self,
    ) -> Result<Mfrc522<SPI, IRQ, Initialized>, Error<SPI::Error, IRQ::Error>> {
        self.reset().await?;
        self.write_register(Register::TxModeReg, 0x00).await?;
        self.write_register(Register::RxModeReg, 0x00).await?;
        self.write_register(Register::ModWidthReg, 0x26).await?;

        // Configure the timer, so we can get a timeout if something goes wrong
        // when communicating with a PICC:
        // - Set timer to start automatically at the end of the transmission
        self.write_register(Register::TModeReg, 0x80).await?;
        // - Configure the prescaler to determine the timer frequency:
        //   f_timer = 13.56 MHz / (2 * TPreScaler + 1)
        //   so for 40kHz frequency (25μs period), TPreScaler = 0x0A9
        self.write_register(Register::TPrescalerReg, 0xA9).await?;
        // - Set the reload value to determine the timeout
        //   for a 25ms timeout, we need a value of 1000 = 0x3E8
        self.write_register(Register::TReloadRegHigh, 0x03).await?;
        self.write_register(Register::TReloadRegLow, 0xE8).await?;

        // TODO: may not be necessary?
        self.write_register(Register::TxASKReg, FORCE_100_ASK)
            .await?;
        // Set preset value of CRC coprocessor according to ISO 14443-3 part 6.2.4
        self.write_register(Register::ModeReg, (0x3f & (!0b11)) | 0b01)
            .await?;
        // Enable antenna
        self.modify_register(Register::TxControlReg, |b| b | 0b11)
            .await?;

        // Clear any pending interrupts first
        self.write_register(Register::ComIrqReg, 0x7F).await?;
        self.write_register(Register::DivIrqReg, 0x7F).await?;

        // Enable interrupts for REQA detection
        self.write_register(Register::ComlEnReg, RX_IRQ | IDLE_IRQ | ERR_IRQ)
            .await?;
        self.write_register(Register::DivlEnReg, CRC_IRQ).await?;

        // Clear interrupts again after enabling to ensure clean state
        self.write_register(Register::ComIrqReg, 0x7F).await?;
        self.write_register(Register::DivIrqReg, 0x7F).await?;

        Ok(Mfrc522 {
            spi: self.spi,
            irq: self.irq,
            state: core::marker::PhantomData,
        })
    }
}

// The public functions can only be used after initializing
impl<SPI, IRQ> Mfrc522<SPI, IRQ, Initialized>
where
    SPI: SpiDevice,
    IRQ: Wait,
{
    /// Send REQA command and wait for response using IRQ
    pub async fn reqa(&mut self) -> Result<AtqA, Error<SPI::Error, IRQ::Error>> {
        // Prepare for next command
        self.reset_to_idle().await?;

        // Write REQA command to FIFO
        self.write_many(Register::FIFODataReg, &[Command::ReqA as u8])
            .await?;

        // Start transceive with 7-bit frame
        self.command(register::Command::Transceive).await?;
        self.write_register(Register::BitFramingReg, (1 << 7) | 7)
            .await?;

        // Wait for IRQ pin to indicate completion with timeout
        self.wait_for_irq_source(50, RX_IRQ | IDLE_IRQ, 0).await?; // 50ms timeout

        // Read FIFO data
        let fifo_level = self.read_register(Register::FIFOLevelReg).await?;
        if fifo_level == 2 {
            let mut buffer = [0u8; 2];
            self.read_many(Register::FIFODataReg, &mut buffer).await?;
            Ok(AtqA { bytes: buffer })
        } else {
            Err(Error::IncompleteFrame)
        }
    }

    /// Has a card been detected?
    pub async fn new_card_present(&mut self) -> Result<AtqA, Error<SPI::Error, IRQ::Error>> {
        self.write_register(Register::TxModeReg, 0x00).await?;
        self.write_register(Register::RxModeReg, 0x00).await?;
        self.write_register(Register::ModWidthReg, 0x26).await?;

        self.reqa().await
    }

    /// Select a PICC after REQA
    pub async fn select(&mut self, atqa: &AtqA) -> Result<Uid, Error<SPI::Error, IRQ::Error>> {
        // Check for proprietary anticollision
        if (atqa.bytes[0] & 0b00011111).count_ones() != 1 {
            return Err(Error::Proprietary);
        }

        // Clear ValuesAfterColl
        self.modify_register(Register::CollReg, |b| b & !0x80)
            .await?;

        let mut cascade_level: u8 = 0;
        let mut uid_bytes: [u8; 10] = [0u8; 10];
        let mut uid_idx: usize = 0;

        let sak = 'cascade: loop {
            let cmd = match cascade_level {
                0 => Command::SelCl1,
                1 => Command::SelCl2,
                2 => Command::SelCl3,
                _ => unreachable!(),
            };
            let mut known_bits = 0;
            let mut tx = [0u8; 9];
            tx[0] = cmd as u8;

            'anticollision: loop {
                let tx_last_bits = known_bits % 8;
                let tx_bytes = 2 + known_bits / 8;
                let end = tx_bytes as usize + if tx_last_bits > 0 { 1 } else { 0 };
                tx[1] = (tx_bytes << 4) + tx_last_bits;

                match self
                    .transceive::<5>(&tx[0..end], tx_last_bits, tx_last_bits)
                    .await
                {
                    Ok(fifo_data) => {
                        fifo_data.copy_bits_to(&mut tx[2..=6], known_bits)?;
                        break 'anticollision;
                    }
                    Err(Error::Collision) => {
                        let coll_reg = self.read_register(Register::CollReg).await?;
                        if coll_reg & (1 << 5) != 0 {
                            return Err(Error::Collision);
                        }
                        let mut coll_pos = coll_reg & 0x1F;
                        if coll_pos == 0 {
                            coll_pos = 32;
                        }
                        if coll_pos < known_bits {
                            return Err(Error::Collision);
                        }
                        let fifo_data = self.fifo_data::<5>().await?;
                        fifo_data.copy_bits_to(&mut tx[2..=6], known_bits)?;
                        known_bits = coll_pos;

                        let count = known_bits % 8;
                        let check_bit = (known_bits - 1) % 8;
                        let index: usize =
                            1 + (known_bits / 8) as usize + if count != 0 { 1 } else { 0 };
                        tx[index] |= 1 << check_bit;
                    }
                    Err(e) => return Err(e),
                }
            }

            // Send select
            tx[1] = 0x70; // NVB: 7 valid bytes
            tx[6] = tx[2] ^ tx[3] ^ tx[4] ^ tx[5]; // BCC

            let crc = self
                .calculate_crc(&[tx[0], tx[1], tx[2], tx[3], tx[4], tx[5], tx[6]])
                .await?;
            tx[7..].copy_from_slice(&crc);

            let rx = self.transceive::<3>(&tx[0..9], 0, 0).await?;
            if rx.valid_bytes != 3 || rx.valid_bits != 0 {
                return Err(Error::IncompleteFrame);
            }

            let sak = Sak::from(rx.buffer[0]);
            let crc_a = &rx.buffer[1..];
            let crc_verify = self.calculate_crc(&[rx.buffer[0]]).await?;
            if crc_a != crc_verify {
                return Err(Error::Crc);
            }

            if !sak.is_complete() {
                uid_bytes[uid_idx..uid_idx + 3].copy_from_slice(&tx[3..6]);
                uid_idx += 3;
                cascade_level += 1;
            } else {
                uid_bytes[uid_idx..uid_idx + 4].copy_from_slice(&tx[2..6]);
                break 'cascade sak;
            }
        };

        match cascade_level {
            0 => Ok(Uid::Single(GenericUid {
                bytes: uid_bytes[0..4].try_into().unwrap(),
                sak,
            })),
            1 => Ok(Uid::Double(GenericUid {
                bytes: uid_bytes[0..7].try_into().unwrap(),
                sak,
            })),
            2 => Ok(Uid::Triple(GenericUid {
                bytes: uid_bytes,
                sak,
            })),
            _ => unreachable!(),
        }
    }

    /// Calculate CRC for data using IRQ-based completion
    async fn calculate_crc(
        &mut self,
        data: &[u8],
    ) -> Result<[u8; 2], Error<SPI::Error, IRQ::Error>> {
        // Prepare for next command
        self.reset_to_idle().await?;

        // Write data to FIFO for CRC calculation
        self.write_many(Register::FIFODataReg, data).await?;

        // Start CRC calculation
        self.command(register::Command::CalcCRC).await?;

        // Wait for IRQ to indicate CRC calculation completion with timeout
        self.wait_for_irq_source(20, 0, CRC_IRQ).await?; // 20ms timeout for CRC

        // CRC calculation complete - read result
        self.command(register::Command::Idle).await?;
        let crc = [
            self.read_register(Register::CRCResultRegLow).await?,
            self.read_register(Register::CRCResultRegHigh).await?,
        ];
        Ok(crc)
    }

    /// Perform transceive operation with IRQ-based completion
    async fn transceive<const RX: usize>(
        &mut self,
        tx_buffer: &[u8],
        tx_last_bits: u8,
        rx_align_bits: u8,
    ) -> Result<FifoData<RX>, Error<SPI::Error, IRQ::Error>>
    where
        [u8; RX]: Sized,
    {
        // Prepare for next command
        self.reset_to_idle().await?;

        // Write data to FIFO
        self.write_many(Register::FIFODataReg, tx_buffer).await?;

        // Configure bit framing register for send/receive
        self.write_register(
            Register::BitFramingReg,
            (1 << 7) | ((rx_align_bits & 0b0111) << 4) | (tx_last_bits & 0b0111),
        )
        .await?;

        // Start transceive command
        self.command(register::Command::Transceive).await?;

        // Wait for IRQ to indicate operation completion with timeout
        self.wait_for_irq_source(50, RX_IRQ | IDLE_IRQ, 0).await?; // 50ms timeout for transceive

        // Successful completion - read FIFO data
        self.fifo_data().await
    }

    /// Get FIFO data
    async fn fifo_data<const RX: usize>(
        &mut self,
    ) -> Result<FifoData<RX>, Error<SPI::Error, IRQ::Error>>
    where
        [u8; RX]: Sized,
    {
        let mut buffer = [0u8; RX];
        let mut valid_bytes = 0;
        let mut valid_bits = 0;

        if RX > 0 {
            valid_bytes = self.read_register(Register::FIFOLevelReg).await? as usize;
            if valid_bytes > RX {
                return Err(Error::NoRoom);
            }
            if valid_bytes > 0 {
                self.read_many(Register::FIFODataReg, &mut buffer[0..valid_bytes])
                    .await?;
                valid_bits = (self.read_register(Register::ControlReg).await? & 0x07) as usize;
            }
        }

        Ok(FifoData {
            buffer,
            valid_bytes,
            valid_bits,
        })
    }

    /// Sets the antenna gain of the receiver
    ///
    /// Setting this to a high value could help if you have issues communicating with a card.
    /// This should increase the *sensitivity* of the antenna, so it is able to detect weaker signals.
    pub async fn set_antenna_gain(
        &mut self,
        gain: RxGain,
    ) -> Result<(), Error<SPI::Error, IRQ::Error>> {
        self.write_register(Register::RFCfgReg, gain.into()).await
    }
}

impl<SPI, IRQ, S> Mfrc522<SPI, IRQ, S>
where
    SPI: SpiDevice,
    IRQ: Wait,
    S: State,
{
    /// Sanitize device state for performing next operation (Idle, IRQ, Fifo)
    async fn reset_to_idle(&mut self) -> Result<(), Error<SPI::Error, IRQ::Error>> {
        self.clear_irq_state().await?;
        self.command(register::Command::Idle).await?;
        self.fifo_flush().await?;
        Ok(())
    }

    /// Check error register
    async fn check_error(&mut self) -> Result<(), Error<SPI::Error, IRQ::Error>> {
        let err = self.read_register(Register::ErrorReg).await?;

        if err & PROTOCOL_ERR != 0 {
            Err(Error::Protocol)
        } else if err & PARITY_ERR != 0 {
            Err(Error::Parity)
        } else if err & CRC_ERR != 0 {
            Err(Error::Crc)
        } else if err & COLL_ERR != 0 {
            Err(Error::Collision)
        } else if err & BUFFER_OVFL != 0 {
            Err(Error::BufferOverflow)
        } else if err & TEMP_ERR != 0 {
            Err(Error::Overheating)
        } else if err & WR_ERR != 0 {
            Err(Error::Wr)
        } else {
            Ok(())
        }
    }

    /// Clear IRQ state and wait for IRQ line to go low
    async fn clear_irq_state(&mut self) -> Result<(), Error<SPI::Error, IRQ::Error>> {
        // Clear interrupt status registers
        self.write_register(Register::ComIrqReg, 0x7F).await?;
        self.write_register(Register::DivIrqReg, 0x7F).await?;

        // Wait for IRQ line to go low (inactive)
        self.irq.wait_for_low().await.map_err(Error::Gpio)?;

        Ok(())
    }

    /// Wait for IRQ line with timeout
    async fn wait_for_irq(&mut self, timeout_ms: u64) -> Result<(), Error<SPI::Error, IRQ::Error>> {
        // Use embassy-time's with_timeout for proper timeout handling
        match self
            .irq
            .wait_for_high()
            .with_timeout(Duration::from_millis(timeout_ms))
            .await
        {
            Ok(irq_result) => irq_result.map_err(Error::Gpio),
            Err(_) => Err(Error::Timeout),
        }
    }

    /// Wait for specific IRQ source with timeout
    async fn wait_for_irq_source(
        &mut self,
        timeout_ms: u64,
        com_irq_mask: u8,
        div_irq_mask: u8,
    ) -> Result<(), Error<SPI::Error, IRQ::Error>> {
        self.wait_for_irq(timeout_ms).await?;

        // Check which interrupt source triggered
        let div_irq = self.read_register(Register::DivIrqReg).await?;
        let com_irq = self.read_register(Register::ComIrqReg).await?;

        if com_irq & ERR_IRQ != 0 {
            // Error occurred
            self.check_error().await?;
            Err(Error::Protocol)
        } else if com_irq & TIMER_IRQ != 0 {
            // Timeout occurred
            Err(Error::Timeout)
        } else if com_irq & com_irq_mask != 0 || div_irq & div_irq_mask != 0 {
            Ok(())
        } else {
            // Unexpected interrupt state
            Err(Error::Protocol)
        }
    }

    /// Returns the version reported by the MFRC522
    pub async fn version(&mut self) -> Result<u8, Error<SPI::Error, IRQ::Error>> {
        self.read_register(Register::VersionReg).await
    }

    /// Release the underlying communication channel
    pub fn release(self) -> (SPI, IRQ) {
        (self.spi, self.irq)
    }

    /// Flush the internal FIFO buffer
    async fn fifo_flush(&mut self) -> Result<(), Error<SPI::Error, IRQ::Error>> {
        self.write_register(Register::FIFOLevelReg, FLUSH_BUFFER)
            .await
    }

    /// Send a command
    async fn command(
        &mut self,
        command: register::Command,
    ) -> Result<(), Error<SPI::Error, IRQ::Error>> {
        self.write_register(Register::CommandReg, command.into())
            .await
    }

    /// Reset the chip
    async fn reset(&mut self) -> Result<(), Error<SPI::Error, IRQ::Error>> {
        self.command(register::Command::SoftReset).await?;
        while self.read_register(Register::CommandReg).await? & POWER_DOWN != 0 {}

        Ok(())
    }

    // SPI helper methods
    async fn read_register(&mut self, reg: Register) -> Result<u8, Error<SPI::Error, IRQ::Error>> {
        let mut buffer = [((reg as u8) << 1) | 0x80, 0];
        self.spi
            .transfer_in_place(&mut buffer)
            .await
            .map_err(Error::Comm)?;
        Ok(buffer[1])
    }

    async fn read_many(
        &mut self,
        reg: Register,
        buffer: &mut [u8],
    ) -> Result<(), Error<SPI::Error, IRQ::Error>> {
        for byte in buffer.iter_mut() {
            *byte = ((reg as u8) << 1) | 0x80;
        }
        if let Some(b) = buffer.last_mut() {
            *b = 0;
        }

        let address = [((reg as u8) << 1) | 0x80];
        let mut operations = [
            Operation::Write(&address),
            Operation::TransferInPlace(buffer),
        ];
        self.spi
            .transaction(&mut operations)
            .await
            .map_err(Error::Comm)?;
        Ok(())
    }

    async fn write_register(
        &mut self,
        reg: Register,
        val: u8,
    ) -> Result<(), Error<SPI::Error, IRQ::Error>> {
        let mut tx = [((reg as u8) << 1) | 0x80, val];
        let rx = [0; 2];
        self.spi.transfer(&mut tx, &rx).await.map_err(Error::Comm)
    }

    async fn write_many(
        &mut self,
        reg: Register,
        bytes: &[u8],
    ) -> Result<(), Error<SPI::Error, IRQ::Error>> {
        let address = [(reg as u8) << 1];
        let mut operations = [Operation::Write(&address), Operation::Write(bytes)];
        self.spi
            .transaction(&mut operations)
            .await
            .map_err(Error::Comm)
    }

    async fn modify_register<F>(
        &mut self,
        reg: Register,
        f: F,
    ) -> Result<(), Error<SPI::Error, IRQ::Error>>
    where
        F: FnOnce(u8) -> u8,
    {
        let current = self.read_register(reg).await?;
        self.write_register(reg, f(current)).await
    }
}

/// Data read from the internal FIFO buffer
#[derive(Debug, PartialEq)]
pub struct FifoData<const L: usize> {
    /// The contents of the FIFO buffer
    pub buffer: [u8; L],
    /// The number of valid bytes in the buffer
    pub valid_bytes: usize,
    /// The number of valid bits in the last byte
    pub valid_bits: usize,
}

impl<const L: usize> FifoData<L> {
    /// Copies FIFO data to destination buffer.
    fn copy_bits_to<SpiE, GpioE>(
        &self,
        dst: &mut [u8],
        dst_valid_bits: u8,
    ) -> Result<u8, Error<SpiE, GpioE>> {
        if self.valid_bytes == 0 {
            return Ok(dst_valid_bits);
        }

        let dst_valid_bytes = dst_valid_bits / 8;
        let dst_valid_last_bits = dst_valid_bits % 8;
        let mask: u8 = 0xFF << dst_valid_last_bits;
        let mut idx = dst_valid_bytes as usize;
        dst[idx] = (self.buffer[0] & mask) | (dst[idx] & !mask);
        idx += 1;
        let len = self.valid_bytes - 1;
        if len + idx > dst.len() {
            return Err(Error::NoRoom);
        }
        if len > 0 {
            dst[idx..idx + len].copy_from_slice(&self.buffer[1..=len]);
        }
        Ok(dst_valid_bits + (self.valid_bytes * 8) as u8 + self.valid_bits as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;

    #[test]
    fn test_atqa_creation() {
        let atqa = AtqA {
            bytes: [0x04, 0x00],
        };
        assert_eq!(atqa.bytes, [0x04, 0x00]);
    }

    #[test]
    fn test_uid_creation() {
        let uid = Uid::Single(GenericUid::new([0x12, 0x34, 0x56, 0x78], 0x08));
        assert_eq!(uid.as_bytes(), &[0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_generic_uid() {
        let uid = GenericUid::new([0x12, 0x34, 0x56, 0x78], 0x08);
        assert_eq!(uid.as_bytes(), &[0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_fifo_data_copy_bits() {
        let fifo_data = FifoData {
            buffer: [0x12, 0x34, 0x00, 0x00, 0x00],
            valid_bytes: 2,
            valid_bits: 0,
        };

        let mut dst = [0u8; 6];
        let result_bits = fifo_data.copy_bits_to::<(), ()>(&mut dst, 0).unwrap();
        assert_eq!(result_bits, 16); // 2 bytes * 8 bits
        assert_eq!(dst[0], 0x12);
        assert_eq!(dst[1], 0x34);
    }

    #[test]
    fn test_picc_command_values() {
        assert_eq!(Command::ReqA as u8, 0x26);
        assert_eq!(Command::SelCl1 as u8, 0x93);
        assert_eq!(Command::SelCl2 as u8, 0x95);
        assert_eq!(Command::SelCl3 as u8, 0x97);
        assert_eq!(Command::CT as u8, 0x88);
    }

    #[test]
    fn test_register_values() {
        assert_eq!(Register::CommandReg as u8, 0x01);
        assert_eq!(Register::ComIrqReg as u8, 0x04);
        assert_eq!(Register::FIFODataReg as u8, 0x09);
        assert_eq!(Register::VersionReg as u8, 0x37);
    }

    #[test]
    fn test_sak_type_detection() {
        // MIFARE Classic 1K
        let sak = Sak::from(0x08);
        assert_eq!(sak.get_type(), Type::Mifare1k);
        assert!(sak.is_complete());
        assert!(!sak.is_compliant());

        // MIFARE Ultralight
        let sak = Sak::from(0x00);
        assert_eq!(sak.get_type(), Type::MifareUL);
        assert!(sak.is_complete());
        assert!(!sak.is_compliant());

        // Not complete (cascade)
        let sak = Sak::from(0x04);
        assert_eq!(sak.get_type(), Type::NotComplete);
        assert!(!sak.is_complete());
    }
}
