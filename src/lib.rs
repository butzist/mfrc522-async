//! Driver library for interfacing with the MFRC522 contacless communication IC,
//! based on the [embedded-hal](https://docs.rs/embedded-hal/latest/embedded_hal/) traits.
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
//! However, currently only SPI communication is implemented in this crate.
//!
//! # Quickstart
//! ```rust
//! // create an SPI device that implements the embedded-hal `spi::Transfer` and `spi::Write` traits
//! let spi = spi::Spi::new(/* */);
//! // create a GPIO output for chip-select control
//! let cs = gpio::Output::new(/* */);
//!
//! let itf = SpiInterface::new(spi).with_nss(cs);
//! let mut mfrc522 = Mfrc522::new(itf).init()?;
//!
//! // The reported version is expected to be 0x91 or 0x92
//! let mfrc522_version = mfrc522.version()?;
//! ```
//!
//! Take a look at [SpiInterface](comm::blocking::spi::SpiInterface) for options when creating
//! the communication interface, and [Mfrc522] for information on the functions
//! that are available after initialization.
//!
//! # Example applications
//!
//! - [Raspberry Pi 4](https://gitlab.com/jspngh/rfid-rs/-/blob/master/examples/rpi.rs)
//! - [Beaglebone Black](https://gitlab.com/jspngh/rfid-rs/-/blob/master/examples/beagle.rs)
//! - [STM32L4](https://gitlab.com/jspngh/stm32l4-mfrc522)

#![cfg_attr(not(feature = "std"), no_std)]

pub mod comm;
pub mod error;
mod picc;
mod register;
mod util;

use comm::Interface;
use error::Error;
use register::*;
use util::Sealed;

const MIFARE_KEYSIZE: usize = 6;
pub type MifareKey = [u8; MIFARE_KEYSIZE];

pub enum Uid {
    /// Single sized UID, 4 bytes long
    Single(GenericUid<4>),
    /// Double sized UID, 7 bytes long
    Double(GenericUid<7>),
    /// Trip sized UID, 10 bytes long
    Triple(GenericUid<10>),
}

impl Uid {
    pub fn as_bytes(&self) -> &[u8] {
        match &self {
            Uid::Single(u) => u.as_bytes(),
            Uid::Double(u) => u.as_bytes(),
            Uid::Triple(u) => u.as_bytes(),
        }
    }
}

pub struct GenericUid<const T: usize>
where
    [u8; T]: Sized,
{
    /// The UID can have 4, 7 or 10 bytes.
    bytes: [u8; T],
    /// The SAK (Select acknowledge) byte returned from the PICC after successful selection.
    sak: picc::Sak,
}

impl<const T: usize> GenericUid<T> {
    pub fn new(bytes: [u8; T], sak_byte: u8) -> Self {
        Self {
            bytes,
            sak: picc::Sak::from(sak_byte),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn is_compliant(&self) -> bool {
        self.sak.is_compliant()
    }
}

/// Answer To reQuest type A
pub struct AtqA {
    bytes: [u8; 2],
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

/// MFRC522 driver
pub struct Mfrc522<COMM: Interface, S: State> {
    comm: COMM,
    state: core::marker::PhantomData<S>,
}

impl<COMM: Interface> Mfrc522<COMM, Uninitialized> {
    /// Create a new MFRC522 driver from the communication interface.
    pub fn new(comm: COMM) -> Self {
        Self {
            comm,
            state: core::marker::PhantomData,
        }
    }
}

impl<COMM: Interface, S: State> Mfrc522<COMM, S> {
    /// Release the underlying communication channel
    pub fn release(self) -> COMM {
        self.comm
    }
}

// The driver can transition to the `Initialized` state using this function
impl<E, COMM: Interface<Error = E>> Mfrc522<COMM, Uninitialized> {
    /// Initialize the MFRC522.
    ///
    /// This needs to be called before you can do any other operation.
    pub fn init(mut self) -> Result<Mfrc522<COMM, Initialized>, Error<E>> {
        self.reset()?;
        self.write(Register::TxModeReg, 0x00)?;
        self.write(Register::RxModeReg, 0x00)?;
        // Reset ModWidthReg to default value
        self.write(Register::ModWidthReg, 0x26)?;

        // Configure the timer, so we can get a timeout if something goes wrong
        // when communicating with a PICC:
        // - Set timer to start automatically at the end of the transmission
        self.write(Register::TModeReg, 0x80)?;
        // - Configure the prescaler to determine the timer frequency:
        //   f_timer = 13.56 MHz / (2 * TPreScaler + 1)
        //   so for 40kHz frequency (25μs period), TPreScaler = 0x0A9
        self.write(Register::TPrescalerReg, 0xA9)?;
        // - Set the reload value to determine the timeout
        //   for a 25ms timeout, we need a value of 1000 = 0x3E8
        self.write(Register::TReloadRegHigh, 0x03)?;
        self.write(Register::TReloadRegLow, 0xE8)?;

        // TODO: may not be necessary?
        self.write(Register::TxASKReg, FORCE_100_ASK)?;
        // Set preset value of CRC coprocessor according to ISO 14443-3 part 6.2.4
        self.write(Register::ModeReg, (0x3f & (!0b11)) | 0b01)?;
        // Enable antenna
        self.rmw(Register::TxControlReg, |b| b | 0b11)?;

        Ok(Mfrc522 {
            comm: self.comm,
            state: core::marker::PhantomData,
        })
    }
}

// The public functions can only be used after initializing
impl<E, COMM: Interface<Error = E>> Mfrc522<COMM, Initialized> {
    /// Sends a REQuest type A to nearby PICCs
    pub fn reqa(&mut self) -> Result<AtqA, Error<E>> {
        // NOTE REQA is a short frame (7 bits)
        let fifo_data = self.transceive(&[picc::Command::ReqA as u8], 7, 0)?;
        if fifo_data.valid_bytes != 2 || fifo_data.valid_bits != 0 {
            Err(Error::IncompleteFrame)
        } else {
            Ok(AtqA {
                bytes: fifo_data.buffer,
            })
        }
    }

    /// Sends a Wake UP type A to nearby PICCs
    pub fn wupa(&mut self) -> Result<AtqA, Error<E>> {
        // NOTE WUPA is a short frame (7 bits)
        let fifo_data = self.transceive(&[picc::Command::WupA as u8], 7, 0)?;
        if fifo_data.valid_bytes != 2 || fifo_data.valid_bits != 0 {
            Err(Error::IncompleteFrame)
        } else {
            Ok(AtqA {
                bytes: fifo_data.buffer,
            })
        }
    }

    /// Sends command to enter HALT state
    pub fn hlta(&mut self) -> Result<(), Error<E>> {
        let mut buffer: [u8; 4] = [picc::Command::HltA as u8, 0, 0, 0];
        let crc = self.calculate_crc(&buffer[..2])?;
        buffer[2..].copy_from_slice(&crc);

        // The standard says:
        //   If the PICC responds with any modulation during a period of 1 ms
        //   after the end of the frame containing the HLTA command,
        //   this response shall be interpreted as 'not acknowledge'.
        // We interpret that this way: only Error::Timeout is a success.
        match self.transceive::<0>(&buffer, 0, 0) {
            Err(Error::Timeout) => Ok(()),
            Ok(_) => Err(Error::Nak),
            Err(e) => Err(e),
        }
    }

    /// Selects a PICC in the READY state
    // TODO add optional UID to select a specific PICC
    pub fn select(&mut self, atqa: &AtqA) -> Result<Uid, Error<E>> {
        // check for proprietary anticollision
        if (atqa.bytes[0] & 0b00011111).count_ones() != 1 {
            return Err(Error::Proprietary);
        }

        // clear `ValuesAfterColl`
        self.rmw(Register::CollReg, |b| b & !0x80)?;

        let mut cascade_level: u8 = 0;
        let mut uid_bytes: [u8; 10] = [0u8; 10];
        let mut uid_idx: usize = 0;

        let sak = 'cascade: loop {
            let cmd = match cascade_level {
                0 => picc::Command::SelCl1,
                1 => picc::Command::SelCl2,
                2 => picc::Command::SelCl3,
                _ => unreachable!(),
            };
            let mut known_bits = 0;
            let mut tx = [0u8; 9];
            tx[0] = cmd as u8;

            // TODO: limit to 32 iterations (as spec dictates)
            'anticollision: loop {
                let tx_last_bits = known_bits % 8;
                let tx_bytes = 2 + known_bits / 8;
                let end = tx_bytes as usize + if tx_last_bits > 0 { 1 } else { 0 };
                tx[1] = (tx_bytes << 4) + tx_last_bits;

                // Tell transceive the only send `tx_last_bits` of the last byte
                // and also to put the first received bit at location `tx_last_bits`.
                // This makes it easier to append the received bits to the uid (in `tx`).
                match self.transceive::<5>(&tx[0..end], tx_last_bits, tx_last_bits) {
                    Ok(fifo_data) => {
                        fifo_data.copy_bits_to(&mut tx[2..=6], known_bits);
                        break 'anticollision;
                    }
                    Err(Error::Collision) => {
                        let coll_reg = self.read(Register::CollReg)?;
                        if coll_reg & (1 << 5) != 0 {
                            // CollPosNotValid
                            return Err(Error::Collision);
                        }
                        let mut coll_pos = coll_reg & 0x1F;
                        if coll_pos == 0 {
                            coll_pos = 32;
                        }
                        if coll_pos < known_bits {
                            // No progress
                            return Err(Error::Collision);
                        }
                        let fifo_data = self.fifo_data::<5>()?;
                        fifo_data.copy_bits_to(&mut tx[2..=6], known_bits);
                        known_bits = coll_pos;

                        // Set the bit of collision position to 1
                        let count = known_bits % 8;
                        let check_bit = (known_bits - 1) % 8;
                        let index: usize =
                            1 + (known_bits / 8) as usize + if count != 0 { 1 } else { 0 };
                        tx[index] |= 1 << check_bit;
                    }
                    Err(e) => return Err(e),
                }
            }

            // send select
            tx[1] = 0x70; // NVB: 7 valid bytes
            tx[6] = tx[2] ^ tx[3] ^ tx[4] ^ tx[5]; // BCC

            let crc = self.calculate_crc(&tx[..7])?;
            tx[7..].copy_from_slice(&crc);

            let rx = self.transceive::<3>(&tx[0..9], 0, 0)?;
            if rx.valid_bytes != 3 || rx.valid_bits != 0 {
                return Err(Error::IncompleteFrame);
            }

            let sak = picc::Sak::from(rx.buffer[0]);
            let crc_a = &rx.buffer[1..];
            let crc_verify = self.calculate_crc(&rx.buffer[..1])?;
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

    /// Switch off the MIFARE Crypto1 unit.
    /// Must be done after communication with an authenticated PICC
    pub fn stop_crypto1(&mut self) -> Result<(), Error<E>> {
        self.rmw(Register::Status2Reg, |b| b & !0x08)
    }

    pub fn mf_authenticate(
        &mut self,
        uid: &Uid,
        block: u8,
        key: &MifareKey,
    ) -> Result<(), Error<E>> {
        // stop any ongoing command
        self.command(Command::Idle)?;
        // clear all interrupt flags
        self.write(Register::ComIrqReg, 0x7f)?;
        // flush FIFO buffer
        self.fifo_flush()?;
        // clear bit framing
        self.write(Register::BitFramingReg, 0)?;

        let mut tx_buffer = [0u8; 12];
        tx_buffer[0] = picc::Command::MfAuthKeyA as u8;
        tx_buffer[1] = block;
        tx_buffer[2..8].copy_from_slice(key);
        match uid {
            Uid::Single(u) => tx_buffer[8..12].copy_from_slice(&u.bytes[0..4]),
            Uid::Double(u) => tx_buffer[8..12].copy_from_slice(&u.bytes[0..4]),
            Uid::Triple(u) => tx_buffer[8..12].copy_from_slice(&u.bytes[0..4]),
        };
        // write data to transmit to the FIFO buffer
        self.write_many(Register::FIFODataReg, &tx_buffer)?;

        // signal command
        self.command(Command::MFAuthent)?;

        let mut irq;
        loop {
            irq = self.read(Register::ComIrqReg)?;

            if irq & (ERR_IRQ | IDLE_IRQ) != 0 {
                break;
            } else if irq & TIMER_IRQ != 0 {
                return Err(Error::Timeout);
            }
        }

        self.check_error_register()?;
        Ok(())
    }

    pub fn mf_read(&mut self, block: u8) -> Result<[u8; 16], Error<E>> {
        let mut tx = [picc::Command::MfRead as u8, block, 0u8, 0u8];

        let crc = self.calculate_crc(&tx[0..2])?;
        tx[2..].copy_from_slice(&crc);

        let rx = self.transceive::<18>(&tx, 0, 0)?.buffer;

        // verify CRC
        let crc = self.calculate_crc(&rx[..16])?;
        if crc != rx[16..] {
            return Err(Error::Crc);
        }
        Ok(rx[..16].try_into().unwrap())
    }

    pub fn mf_write(&mut self, block: u8, data: [u8; 16]) -> Result<(), Error<E>> {
        let mut cmd = [picc::Command::MfWrite as u8, block, 0, 0];
        let crc = self.calculate_crc(&cmd[0..2])?;
        cmd[2..].copy_from_slice(&crc);
        let fifo_data = self.transceive::<1>(&cmd, 0, 0)?;
        if fifo_data.valid_bytes != 1 || fifo_data.valid_bits != 4 {
            return Err(Error::Nak);
        }

        let mut tx = [0u8; 18];
        let crc = self.calculate_crc(&data)?;
        tx[..16].copy_from_slice(&data);
        tx[16..].copy_from_slice(&crc);
        let fifo_data = self.transceive::<1>(&tx, 0, 0)?;
        if fifo_data.valid_bytes != 1 || fifo_data.valid_bits != 4 {
            return Err(Error::Nak);
        }

        Ok(())
    }

    /// Returns the version reported by the MFRC522
    pub fn version(&mut self) -> Result<u8, Error<E>> {
        self.read(Register::VersionReg)
    }

    pub fn new_card_present(&mut self) -> Result<AtqA, Error<E>> {
        self.write(Register::TxModeReg, 0x00)?;
        self.write(Register::RxModeReg, 0x00)?;
        self.write(Register::ModWidthReg, 0x26)?;

        self.reqa()
    }
}

// The private functions are implemented for all states.
impl<E, COMM: Interface<Error = E>, S: State> Mfrc522<COMM, S> {
    fn calculate_crc(&mut self, data: &[u8]) -> Result<[u8; 2], Error<E>> {
        // stop any ongoing command
        self.command(Command::Idle)?;

        // clear the CRC_IRQ interrupt flag
        self.write(Register::DivIrqReg, 1 << 2)?;

        // flush FIFO buffer
        self.fifo_flush()?;

        // write data to transmit to the FIFO buffer
        self.write_many(Register::FIFODataReg, data)?;

        self.command(Command::CalcCRC)?;

        // Wait for the CRC calculation to complete.
        let mut irq;
        for _ in 0..5000 {
            irq = self.read(Register::DivIrqReg)?;

            if irq & CRC_IRQ != 0 {
                self.command(Command::Idle)?;
                let crc = [
                    self.read(Register::CRCResultRegLow)?,
                    self.read(Register::CRCResultRegHigh)?,
                ];

                return Ok(crc);
            }
        }
        Err(Error::Timeout)
    }

    fn check_error_register(&mut self) -> Result<(), Error<E>> {
        let err = self.read(Register::ErrorReg)?;

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

    // Transmit + Receive
    fn transceive<const RX: usize>(
        &mut self,
        // the data to be sent
        tx_buffer: &[u8],
        // number of bits in the last byte that will be transmitted
        tx_last_bits: u8,
        // bit position for the first received bit to be stored in the FIFO buffer
        rx_align_bits: u8,
    ) -> Result<FifoData<RX>, Error<E>>
    where
        [u8; RX]: Sized,
    {
        // stop any ongoing command
        self.command(Command::Idle)?;

        // clear all interrupt flags
        self.write(Register::ComIrqReg, 0x7f)?;

        // flush FIFO buffer
        self.fifo_flush()?;

        // write data to transmit to the FIFO buffer
        self.write_many(Register::FIFODataReg, tx_buffer)?;

        // signal command
        self.command(Command::Transceive)?;

        // configure short frame and start transmission
        self.write(
            Register::BitFramingReg,
            (1 << 7) | ((rx_align_bits & 0b0111) << 4) | (tx_last_bits & 0b0111),
        )?;

        // TODO timeout when connection to the MFRC522 is lost (?)
        // wait for transmission + reception to complete
        loop {
            let irq = self.read(Register::ComIrqReg)?;

            if irq & (RX_IRQ | ERR_IRQ | IDLE_IRQ) != 0 {
                break;
            } else if irq & TIMER_IRQ != 0 {
                return Err(Error::Timeout);
            }
        }

        self.check_error_register()?;
        self.fifo_data()
    }

    /// Get the data from the internal FIFO buffer
    fn fifo_data<const RX: usize>(&mut self) -> Result<FifoData<RX>, Error<E>> {
        let mut buffer = [0u8; RX];
        let mut valid_bytes = 0;
        let mut valid_bits = 0;

        if RX > 0 {
            valid_bytes = self.read(Register::FIFOLevelReg)? as usize;
            if valid_bytes > RX {
                return Err(Error::NoRoom);
            }
            if valid_bytes > 0 {
                self.read_many(Register::FIFODataReg, &mut buffer[0..valid_bytes])?;
                valid_bits = (self.read(Register::ControlReg)? & 0x07) as usize;
            }
        }

        Ok(FifoData {
            buffer,
            valid_bytes,
            valid_bits,
        })
    }

    /// Flush the internal FIFO buffer
    fn fifo_flush(&mut self) -> Result<(), Error<E>> {
        self.write(Register::FIFOLevelReg, FLUSH_BUFFER)
    }

    /// Request to execute the given command
    fn command(&mut self, command: Command) -> Result<(), Error<E>> {
        self.write(Register::CommandReg, command.into())
    }

    /// Perform a software reset
    fn reset(&mut self) -> Result<(), Error<E>> {
        self.command(Command::SoftReset)?;
        while self.read(Register::CommandReg)? & POWER_DOWN != 0 {}
        Ok(())
    }

    // Convenience wrappers for the `Interface` methods

    fn read(&mut self, reg: Register) -> Result<u8, Error<E>> {
        self.comm.read(reg).map_err(Error::Comm)
    }

    fn read_many<'b>(&mut self, reg: Register, buffer: &'b mut [u8]) -> Result<&'b [u8], Error<E>> {
        self.comm.read_many(reg, buffer).map_err(Error::Comm)
    }

    fn write(&mut self, reg: Register, val: u8) -> Result<(), Error<E>> {
        self.comm.write(reg, val).map_err(Error::Comm)
    }

    fn write_many(&mut self, reg: Register, bytes: &[u8]) -> Result<(), Error<E>> {
        self.comm.write_many(reg, bytes).map_err(Error::Comm)
    }

    fn rmw<F>(&mut self, reg: Register, f: F) -> Result<(), Error<E>>
    where
        F: FnOnce(u8) -> u8,
    {
        self.comm.rmw(reg, f).map_err(Error::Comm)
    }
}

#[derive(Debug, PartialEq)]
struct FifoData<const L: usize> {
    /// The contents of the FIFO buffer
    buffer: [u8; L],
    /// The number of valid bytes in the buffer
    valid_bytes: usize,
    /// The number of valid bits in the last byte
    valid_bits: usize,
}

impl<const L: usize> FifoData<L> {
    /// Copies FIFO data to destination buffer.
    /// Assumes the FIFO data is aligned properly to append directly to the current known bits.
    /// Returns the number of valid bits in the destination buffer after copy.
    pub fn copy_bits_to(&self, dst: &mut [u8], dst_valid_bits: u8) -> u8 {
        if self.valid_bytes == 0 {
            // nothing to copy
            return dst_valid_bits;
        }

        let dst_valid_bytes = dst_valid_bits / 8;
        let dst_valid_last_bits = dst_valid_bits % 8;
        let mask: u8 = 0xFF << dst_valid_last_bits;
        let mut idx = dst_valid_bytes as usize;
        dst[idx] = (self.buffer[0] & mask) | (dst[idx] & !mask);
        idx += 1;
        let len = self.valid_bytes - 1;
        if len > 0 {
            dst[idx..idx + len].copy_from_slice(&self.buffer[1..=len]);
        }
        dst_valid_bits + (len * 8) as u8 + self.valid_bits as u8
    }
}

#[cfg(test)]
mod test {
    use crate::comm::eh02::spi::SpiInterface;
    use crate::Mfrc522;
    use embedded_hal_mock_02::spi::{Mock as SpiMock, Transaction as SpiTransaction};

    #[test]
    pub fn test_calculate_crc() {
        let expectations = [
            SpiTransaction::write([0x02, 0x00].to_vec()),
            SpiTransaction::write([0x0A, 0x04].to_vec()),
            SpiTransaction::write([0x14, 0x80].to_vec()),
            SpiTransaction::write([0x12, 0x01, 0x02, 0x40].to_vec()),
            SpiTransaction::write([0x02, 0x03].to_vec()),
            SpiTransaction::transfer([0x8A, 0x00].to_vec(), [0x23, 0x00].to_vec()),
            SpiTransaction::transfer([0x8A, 0x00].to_vec(), [0x32, 0x1B].to_vec()),
            SpiTransaction::transfer([0x8A, 0x00].to_vec(), [0x63, 0x1F].to_vec()),
            SpiTransaction::write([0x02, 0x00].to_vec()),
            SpiTransaction::transfer([0xC4, 0x00].to_vec(), [0x29, 0xbe].to_vec()),
            SpiTransaction::transfer([0xC2, 0x00].to_vec(), [0x93, 0xef].to_vec()),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).calculate_crc(&[0x01, 0x02, 0x40]),
            Ok([0xbe, 0xef])
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_check_error_register_protocol() {
        let expectations = [SpiTransaction::transfer(
            [0x8C, 0x00].to_vec(),
            [0xef, 0x01].to_vec(),
        )];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).check_error_register(),
            Err(crate::error::Error::Protocol)
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_check_error_crc_err() {
        let expectations = [SpiTransaction::transfer(
            [0x8C, 0x00].to_vec(),
            [0xef, 0x04].to_vec(),
        )];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).check_error_register(),
            Err(crate::error::Error::Crc)
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_check_error_ok() {
        let expectations = [SpiTransaction::transfer(
            [0x8C, 0x00].to_vec(),
            [0xef, 0x00].to_vec(),
        )];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(Mfrc522::new(spi).check_error_register(), Ok(()));

        spi_clone.done();
    }

    #[test]
    pub fn test_transceive_rx_0() {
        let expectations = [
            SpiTransaction::write([0x02, 0x00].to_vec()),
            SpiTransaction::write([0x08, 0x7f].to_vec()),
            SpiTransaction::write([0x14, 0x80].to_vec()),
            SpiTransaction::write([0x12, 0xfe, 0xed].to_vec()),
            SpiTransaction::write([0x02, 0x0C].to_vec()),
            SpiTransaction::write([0x1A, 0xA1].to_vec()),
            SpiTransaction::transfer([0x88, 0x00].to_vec(), [0x39, 0x04].to_vec()),
            SpiTransaction::transfer([0x88, 0x00].to_vec(), [0x71, 0x02].to_vec()),
            SpiTransaction::transfer([0x8C, 0x00].to_vec(), [0xef, 0x00].to_vec()),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi)
            .transceive::<0>(&[0xfe, 0xed], 0xf9, 0xfa)
            .unwrap();

        spi_clone.done();
    }

    #[test]
    pub fn test_transceive() {
        let expectations = [
            SpiTransaction::write([0x02, 0x00].to_vec()),
            SpiTransaction::write([0x08, 0x7f].to_vec()),
            SpiTransaction::write([0x14, 0x80].to_vec()),
            SpiTransaction::write([0x12, 0xfe, 0xed].to_vec()),
            SpiTransaction::write([0x02, 0x0C].to_vec()),
            SpiTransaction::write([0x1A, 0xA1].to_vec()),
            SpiTransaction::transfer([0x88, 0x00].to_vec(), [0x39, 0x04].to_vec()),
            SpiTransaction::transfer([0x88, 0x00].to_vec(), [0x71, 0x02].to_vec()),
            SpiTransaction::transfer([0x8C, 0x00].to_vec(), [0xef, 0x00].to_vec()),
            SpiTransaction::transfer([0x94, 0x00].to_vec(), [0xe0, 0x04].to_vec()),
            SpiTransaction::transfer(
                [0x92, 0x92, 0x92, 0x92, 0x00].to_vec(),
                [0x91, 0x98, 0x76, 0x52, 0x2b].to_vec(),
            ),
            SpiTransaction::transfer([0x98, 0x00].to_vec(), [0x30, 0xf3].to_vec()),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).transceive::<4>(&[0xfe, 0xed], 0xf9, 0xfa),
            Ok(crate::FifoData {
                buffer: [0x98, 0x76, 0x52, 0x2b],
                valid_bytes: 4,
                valid_bits: 3,
            })
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_fifo_data_0() {
        let spi = SpiMock::new(&[]);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).fifo_data::<0>(),
            Ok(crate::FifoData {
                buffer: [],
                valid_bytes: 0,
                valid_bits: 0,
            })
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_fifo_data_1() {
        let spi = SpiMock::new(&[
            SpiTransaction::transfer([0x94, 0x00].to_vec(), [0xe0, 0x01].to_vec()),
            SpiTransaction::transfer([0x92, 0x00].to_vec(), [0x91, 0x98].to_vec()),
            SpiTransaction::transfer([0x98, 0x00].to_vec(), [0x30, 0x07].to_vec()),
        ]);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).fifo_data::<1>(),
            Ok(crate::FifoData {
                buffer: [0x98],
                valid_bytes: 1,
                valid_bits: 7,
            })
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_fifo_flush() {
        let expectations = [SpiTransaction::write([0x14, 0x80].to_vec())];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi).fifo_flush().unwrap(); //.init().unwrap();

        spi_clone.done();
    }

    #[test]
    pub fn test_command() {
        let expectations = [SpiTransaction::write([0x02, 0x0E].to_vec())];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi)
            .command(crate::register::Command::MFAuthent)
            .unwrap(); //.init().unwrap();

        spi_clone.done();
    }

    #[test]
    pub fn test_reset() {
        let expectations = [
            SpiTransaction::write([0x02, 0x0F].to_vec()),
            SpiTransaction::transfer([0x82, 0x00].to_vec(), [0xec, 0x30].to_vec()),
            SpiTransaction::transfer([0x82, 0x00].to_vec(), [0xce, 0x10].to_vec()),
            SpiTransaction::transfer([0x82, 0x00].to_vec(), [0xbe, 0x0f].to_vec()),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi).reset().unwrap(); //.init().unwrap();

        spi_clone.done();
    }

    #[test]
    pub fn test_read() {
        let expectations = [SpiTransaction::transfer(
            [0x96, 0x00].to_vec(),
            [0x11, 0x37].to_vec(),
        )];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).read(crate::register::Register::WaterLevelReg),
            Ok(0x37)
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_read_many_2() {
        let expectations = [SpiTransaction::transfer(
            [0xAA, 0xAA, 0x00].to_vec(),
            [0x69, 0x12, 0x23].to_vec(),
        )];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        let mut buffer = [0u8; 2];
        Mfrc522::new(spi)
            .read_many(crate::register::Register::TxASKReg, &mut buffer)
            .unwrap();
        assert_eq!(buffer, [0x12, 0x23]);

        spi_clone.done();
    }

    #[test]
    pub fn test_read_many_3() {
        let expectations = [SpiTransaction::transfer(
            [0xAA, 0xAA, 0xAA, 0x00].to_vec(),
            [0x69, 0x12, 0x23, 0x34].to_vec(),
        )];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        let mut buffer = [0u8; 3];
        Mfrc522::new(spi)
            .read_many(crate::register::Register::TxASKReg, &mut buffer)
            .unwrap();
        assert_eq!(buffer, [0x12, 0x23, 0x34]);

        spi_clone.done();
    }

    #[test]
    pub fn test_rmw() {
        let expectations = [
            SpiTransaction::transfer([0xA6, 0x00].to_vec(), [0xdb, 0x37].to_vec()),
            SpiTransaction::write([0x26, 0xC8].to_vec()),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi)
            .rmw(crate::register::Register::RxModeReg, |b| !b)
            .unwrap();

        spi_clone.done();
    }

    #[test]
    pub fn test_write() {
        let expectations = [SpiTransaction::write([0x42, 0xfd].to_vec())];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi)
            .write(crate::register::Register::CRCResultRegHigh, 0xfd)
            .unwrap();

        spi_clone.done();
    }

    #[test]
    pub fn test_write_many() {
        let expectations = [SpiTransaction::write(
            [0x4E, 0xca, 0xfe, 0xf0, 0x0d].to_vec(),
        )];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi)
            .write_many(crate::register::Register::GsNReg, &[0xca, 0xfe, 0xf0, 0x0d])
            .unwrap();

        spi_clone.done();
    }
}

#[cfg(test)]
mod test_eh02_i2c {
    use crate::comm::eh02::i2c::I2cInterface;
    use crate::Mfrc522;
    use embedded_hal_mock_02::i2c::{Mock as I2cMock, Transaction as I2cTransaction};

    #[test]
    pub fn test_calculate_crc() {
        let expectations = [
            I2cTransaction::write(0x2C, [0x01, 0x00].to_vec()),
            I2cTransaction::write(0x2C, [0x05, 0x04].to_vec()),
            I2cTransaction::write(0x2C, [0x0A, 0x80].to_vec()),
            I2cTransaction::write(0x2C, [0x09, 0x01, 0x02, 0x40].to_vec()),
            I2cTransaction::write(0x2C, [0x01, 0x03].to_vec()),
            I2cTransaction::write_read(0x2C, [0x05].to_vec(), [0x00].to_vec()),
            I2cTransaction::write_read(0x2C, [0x05].to_vec(), [0x1B].to_vec()),
            I2cTransaction::write_read(0x2C, [0x05].to_vec(), [0x1F].to_vec()),
            I2cTransaction::write(0x2C, [0x01, 0x00].to_vec()),
            I2cTransaction::write_read(0x2C, [0x22].to_vec(), [0xbe].to_vec()),
            I2cTransaction::write_read(0x2C, [0x21].to_vec(), [0xef].to_vec()),
        ];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).calculate_crc(&[0x01, 0x02, 0x40]),
            Ok([0xbe, 0xef])
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_check_error_register_protocol() {
        let expectations = [I2cTransaction::write_read(
            0x2C,
            [0x06].to_vec(),
            [0x01].to_vec(),
        )];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).check_error_register(),
            Err(crate::error::Error::Protocol)
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_check_error_crc_err() {
        let expectations = [I2cTransaction::write_read(
            0x2C,
            [0x06].to_vec(),
            [0x04].to_vec(),
        )];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).check_error_register(),
            Err(crate::error::Error::Crc)
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_check_error_ok() {
        let expectations = [I2cTransaction::write_read(
            0x2C,
            [0x06].to_vec(),
            [0x00].to_vec(),
        )];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(Mfrc522::new(i2c).check_error_register(), Ok(()));

        i2c_clone.done();
    }

    #[test]
    pub fn test_transceive_rx_0() {
        let expectations = [
            I2cTransaction::write(0x2C, [0x01, 0x00].to_vec()),
            I2cTransaction::write(0x2C, [0x04, 0x7f].to_vec()),
            I2cTransaction::write(0x2C, [0x0A, 0x80].to_vec()),
            I2cTransaction::write(0x2C, [0x09, 0xfe, 0xed].to_vec()),
            I2cTransaction::write(0x2C, [0x01, 0x0C].to_vec()),
            I2cTransaction::write(0x2C, [0x0D, 0xA1].to_vec()),
            I2cTransaction::write_read(0x2C, [0x04].to_vec(), [0x04].to_vec()),
            I2cTransaction::write_read(0x2C, [0x04].to_vec(), [0x02].to_vec()),
            I2cTransaction::write_read(0x2C, [0x06].to_vec(), [0x00].to_vec()),
        ];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c)
            .transceive::<0>(&[0xfe, 0xed], 0xf9, 0xfa)
            .unwrap();

        i2c_clone.done();
    }

    #[test]
    pub fn test_transceive() {
        let expectations = [
            I2cTransaction::write(0x2C, [0x01, 0x00].to_vec()),
            I2cTransaction::write(0x2C, [0x04, 0x7f].to_vec()),
            I2cTransaction::write(0x2C, [0x0A, 0x80].to_vec()),
            I2cTransaction::write(0x2C, [0x09, 0xfe, 0xed].to_vec()),
            I2cTransaction::write(0x2C, [0x01, 0x0C].to_vec()),
            I2cTransaction::write(0x2C, [0x0D, 0xA1].to_vec()),
            I2cTransaction::write_read(0x2C, [0x04].to_vec(), [0x04].to_vec()),
            I2cTransaction::write_read(0x2C, [0x04].to_vec(), [0x02].to_vec()),
            I2cTransaction::write_read(0x2C, [0x06].to_vec(), [0x00].to_vec()),
            I2cTransaction::write_read(0x2C, [0x0A].to_vec(), [0x04].to_vec()),
            I2cTransaction::write_read(0x2C, [0x09].to_vec(), [0x98, 0x76, 0x52, 0x2b].to_vec()),
            I2cTransaction::write_read(0x2C, [0x0C].to_vec(), [0x91].to_vec()),
        ];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).transceive::<4>(&[0xfe, 0xed], 0xf9, 0xfa),
            Ok(crate::FifoData {
                buffer: [0x98, 0x76, 0x52, 0x2b],
                valid_bytes: 4,
                valid_bits: 1,
            })
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_fifo_data_0() {
        let i2c = I2cMock::new(&[]);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).fifo_data::<0>(),
            Ok(crate::FifoData {
                buffer: [],
                valid_bytes: 0,
                valid_bits: 0,
            })
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_fifo_data_1() {
        let i2c = I2cMock::new(&[
            I2cTransaction::write_read(0x2C, [0x0A].to_vec(), [0x01].to_vec()),
            I2cTransaction::write_read(0x2C, [0x09].to_vec(), [0x98].to_vec()),
            I2cTransaction::write_read(0x2C, [0x0C].to_vec(), [0x07].to_vec()),
        ]);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).fifo_data::<1>(),
            Ok(crate::FifoData {
                buffer: [0x98],
                valid_bytes: 1,
                valid_bits: 7,
            })
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_fifo_flush() {
        let expectations = [I2cTransaction::write(0x2C, [0x0A, 0x80].to_vec())];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c).fifo_flush().unwrap(); //.init().unwrap();

        i2c_clone.done();
    }

    #[test]
    pub fn test_command() {
        let expectations = [I2cTransaction::write(0x2C, [0x01, 0x0E].to_vec())];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c)
            .command(crate::register::Command::MFAuthent)
            .unwrap(); //.init().unwrap();

        i2c_clone.done();
    }

    #[test]
    pub fn test_reset() {
        let expectations = [
            I2cTransaction::write(0x2C, [0x01, 0x0F].to_vec()),
            I2cTransaction::write_read(0x2C, [0x01].to_vec(), [0x30].to_vec()),
            I2cTransaction::write_read(0x2C, [0x01].to_vec(), [0x10].to_vec()),
            I2cTransaction::write_read(0x2C, [0x01].to_vec(), [0x0f].to_vec()),
        ];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c).reset().unwrap(); //.init().unwrap();

        i2c_clone.done();
    }

    #[test]
    pub fn test_read() {
        let expectations = [I2cTransaction::write_read(
            0x2C,
            [0x0B].to_vec(),
            [0x37].to_vec(),
        )];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).read(crate::register::Register::WaterLevelReg),
            Ok(0x37)
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_read_many_2() {
        let expectations = [I2cTransaction::write_read(
            0x2C,
            [0x15].to_vec(),
            [0x12, 0x23].to_vec(),
        )];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        let mut buffer = [0u8; 2];
        Mfrc522::new(i2c)
            .read_many(crate::register::Register::TxASKReg, &mut buffer)
            .unwrap();
        assert_eq!(buffer, [0x12, 0x23]);

        i2c_clone.done();
    }

    #[test]
    pub fn test_read_many_3() {
        let expectations = [I2cTransaction::write_read(
            0x2C,
            [0x15].to_vec(),
            [0x12, 0x23, 0x34].to_vec(),
        )];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        let mut buffer = [0u8; 3];
        Mfrc522::new(i2c)
            .read_many(crate::register::Register::TxASKReg, &mut buffer)
            .unwrap();
        assert_eq!(buffer, [0x12, 0x23, 0x34]);

        i2c_clone.done();
    }

    #[test]
    pub fn test_rmw() {
        let expectations = [
            I2cTransaction::write_read(0x2C, [0x13].to_vec(), [0x37].to_vec()),
            I2cTransaction::write(0x2C, [0x13, 0xC8].to_vec()),
        ];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c)
            .rmw(crate::register::Register::RxModeReg, |b| !b)
            .unwrap();

        i2c_clone.done();
    }

    #[test]
    pub fn test_write() {
        let expectations = [I2cTransaction::write(0x2C, [0x21, 0xfd].to_vec())];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c)
            .write(crate::register::Register::CRCResultRegHigh, 0xfd)
            .unwrap();

        i2c_clone.done();
    }

    #[test]
    pub fn test_write_many() {
        let expectations = [I2cTransaction::write(
            0x2C,
            [0x27, 0xca, 0xfe, 0xf0, 0x0d].to_vec(),
        )];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c)
            .write_many(crate::register::Register::GsNReg, &[0xca, 0xfe, 0xf0, 0x0d])
            .unwrap();

        i2c_clone.done();
    }
}

#[cfg(all(test, feature = "eh1"))]
mod test_spi {
    use crate::comm::blocking::spi::SpiInterface;
    use crate::Mfrc522;
    use embedded_hal_mock_1::spi::{Mock as SpiMock, Transaction as SpiTransaction};

    #[test]
    pub fn test_calculate_crc() {
        let expectations = [
            SpiTransaction::write_vec([0x02, 0x00].to_vec()),
            SpiTransaction::write_vec([0x0A, 0x04].to_vec()),
            SpiTransaction::write_vec([0x14, 0x80].to_vec()),
            SpiTransaction::transaction_start(),
            SpiTransaction::write_vec([0x12].to_vec()),
            SpiTransaction::write_vec([0x01, 0x02, 0x40].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::write_vec([0x02, 0x03].to_vec()),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x8A, 0x00].to_vec(), [0x23, 0x00].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x8A, 0x00].to_vec(), [0x32, 0x1B].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x8A, 0x00].to_vec(), [0x63, 0x1F].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::write_vec([0x02, 0x00].to_vec()),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0xC4, 0x00].to_vec(), [0x29, 0xbe].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0xC2, 0x00].to_vec(), [0x93, 0xef].to_vec()),
            SpiTransaction::transaction_end(),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).calculate_crc(&[0x01, 0x02, 0x40]),
            Ok([0xbe, 0xef])
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_check_error_register_protocol() {
        let expectations = [
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x8C, 0x00].to_vec(), [0xef, 0x01].to_vec()),
            SpiTransaction::transaction_end(),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).check_error_register(),
            Err(crate::error::Error::Protocol)
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_check_error_crc_err() {
        let expectations = [
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x8C, 0x00].to_vec(), [0xef, 0x04].to_vec()),
            SpiTransaction::transaction_end(),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).check_error_register(),
            Err(crate::error::Error::Crc)
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_check_error_ok() {
        let expectations = [
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x8C, 0x00].to_vec(), [0xef, 0x00].to_vec()),
            SpiTransaction::transaction_end(),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(Mfrc522::new(spi).check_error_register(), Ok(()));

        spi_clone.done();
    }

    #[test]
    pub fn test_transceive_rx_0() {
        let expectations = [
            SpiTransaction::write_vec([0x02, 0x00].to_vec()),
            SpiTransaction::write_vec([0x08, 0x7f].to_vec()),
            SpiTransaction::write_vec([0x14, 0x80].to_vec()),
            SpiTransaction::write_vec([0x12, 0xfe, 0xed].to_vec()),
            SpiTransaction::write_vec([0x02, 0x0C].to_vec()),
            SpiTransaction::write_vec([0x1A, 0xA1].to_vec()),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x88, 0x00].to_vec(), [0x39, 0x04].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x88, 0x00].to_vec(), [0x71, 0x02].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x8C, 0x00].to_vec(), [0xef, 0x00].to_vec()),
            SpiTransaction::transaction_end(),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi)
            .transceive::<0>(&[0xfe, 0xed], 0xf9, 0xfa)
            .unwrap();

        spi_clone.done();
    }

    #[test]
    pub fn test_transceive() {
        let expectations = [
            SpiTransaction::write_vec([0x02, 0x00].to_vec()),
            SpiTransaction::write_vec([0x08, 0x7f].to_vec()),
            SpiTransaction::write_vec([0x14, 0x80].to_vec()),
            SpiTransaction::transaction_start(),
            SpiTransaction::write_vec([0x12].to_vec()),
            SpiTransaction::write_vec([0xfe, 0xed].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::write_vec([0x02, 0x0C].to_vec()),
            SpiTransaction::write_vec([0x1A, 0xA1].to_vec()),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x88, 0x00].to_vec(), [0x00, 0x04].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x88, 0x00].to_vec(), [0x00, 0x02].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x8C, 0x00].to_vec(), [0x00, 0x00].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x94, 0x00].to_vec(), [0x00, 0x04].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::write(0x92),
            SpiTransaction::transfer_in_place(
                [0x92, 0x92, 0x92, 0x00].to_vec(),
                [0x98, 0x76, 0x52, 0x2b].to_vec(),
            ),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x98, 0x00].to_vec(), [0x00, 0xf3].to_vec()),
            SpiTransaction::transaction_end(),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).transceive::<4>(&[0xfe, 0xed], 0xf9, 0xfa),
            Ok(crate::FifoData {
                buffer: [0x98, 0x76, 0x52, 0x2b],
                valid_bytes: 4,
                valid_bits: 3,
            })
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_fifo_data_0() {
        let spi = SpiMock::new(&[]);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).fifo_data::<0>(),
            Ok(crate::FifoData {
                buffer: [],
                valid_bytes: 0,
                valid_bits: 0,
            })
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_fifo_data_1() {
        let spi = SpiMock::new(&[
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x94, 0x00].to_vec(), [0xe0, 0x01].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x92, 0x00].to_vec(), [0x91, 0x98].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x98, 0x00].to_vec(), [0x30, 0x07].to_vec()),
            SpiTransaction::transaction_end(),
        ]);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).fifo_data::<1>(),
            Ok(crate::FifoData {
                buffer: [0x98],
                valid_bytes: 1,
                valid_bits: 7,
            })
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_fifo_flush() {
        let expectations = [SpiTransaction::write_vec([0x14, 0x80].to_vec())];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi).fifo_flush().unwrap(); //.init().unwrap();

        spi_clone.done();
    }

    #[test]
    pub fn test_command() {
        let expectations = [SpiTransaction::write_vec([0x02, 0x0E].to_vec())];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi)
            .command(crate::register::Command::MFAuthent)
            .unwrap(); //.init().unwrap();

        spi_clone.done();
    }

    #[test]
    pub fn test_reset() {
        let expectations = [
            SpiTransaction::write_vec([0x02, 0x0F].to_vec()),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x82, 0x00].to_vec(), [0xec, 0x30].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x82, 0x00].to_vec(), [0xce, 0x10].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x82, 0x00].to_vec(), [0xbe, 0x0f].to_vec()),
            SpiTransaction::transaction_end(),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi).reset().unwrap(); //.init().unwrap();

        spi_clone.done();
    }

    #[test]
    pub fn test_read() {
        let expectations = [
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0x96, 0x00].to_vec(), [0x11, 0x37].to_vec()),
            SpiTransaction::transaction_end(),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        assert_eq!(
            Mfrc522::new(spi).read(crate::register::Register::WaterLevelReg),
            Ok(0x37)
        );

        spi_clone.done();
    }

    #[test]
    pub fn test_read_many_2() {
        let expectations = [
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place(
                [0xAA, 0xAA, 0x00].to_vec(),
                [0x69, 0x12, 0x23].to_vec(),
            ),
            SpiTransaction::transaction_end(),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        let mut buffer = [0u8; 2];
        Mfrc522::new(spi)
            .read_many(crate::register::Register::TxASKReg, &mut buffer)
            .unwrap();
        assert_eq!(buffer, [0x12, 0x23]);

        spi_clone.done();
    }

    #[test]
    pub fn test_read_many_3() {
        let expectations = [
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place(
                [0xAA, 0xAA, 0xAA, 0x00].to_vec(),
                [0x69, 0x12, 0x23, 0x34].to_vec(),
            ),
            SpiTransaction::transaction_end(),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        let mut buffer = [0u8; 3];
        Mfrc522::new(spi)
            .read_many(crate::register::Register::TxASKReg, &mut buffer)
            .unwrap();
        assert_eq!(buffer, [0x12, 0x23, 0x34]);

        spi_clone.done();
    }

    #[test]
    pub fn test_rmw() {
        let expectations = [
            SpiTransaction::transaction_start(),
            SpiTransaction::transfer_in_place([0xA6, 0x00].to_vec(), [0xdb, 0x37].to_vec()),
            SpiTransaction::transaction_end(),
            SpiTransaction::write_vec([0x26, 0xC8].to_vec()),
        ];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi)
            .rmw(crate::register::Register::RxModeReg, |b| !b)
            .unwrap();

        spi_clone.done();
    }

    #[test]
    pub fn test_write() {
        let expectations = [SpiTransaction::write_vec([0x42, 0xfd].to_vec())];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi)
            .write(crate::register::Register::CRCResultRegHigh, 0xfd)
            .unwrap();

        spi_clone.done();
    }

    #[test]
    pub fn test_write_many() {
        let expectations = [SpiTransaction::write_vec(
            [0x4E, 0xca, 0xfe, 0xf0, 0x0d].to_vec(),
        )];

        let spi = SpiMock::new(&expectations);
        let mut spi_clone = spi.clone();
        let spi = SpiInterface::new(spi);

        Mfrc522::new(spi)
            .write_many(crate::register::Register::GsNReg, &[0xca, 0xfe, 0xf0, 0x0d])
            .unwrap();

        spi_clone.done();
    }
}

#[cfg(all(test, feature = "eh1"))]
mod test_i2c {
    use crate::comm::blocking::i2c::I2cInterface;
    use crate::Mfrc522;
    use embedded_hal_mock_1::i2c::{Mock as I2cMock, Transaction as I2cTransaction};

    #[test]
    pub fn test_calculate_crc() {
        let expectations = [
            I2cTransaction::write(0x2C, [0x01, 0x00].to_vec()),
            I2cTransaction::write(0x2C, [0x05, 0x04].to_vec()),
            I2cTransaction::write(0x2C, [0x0A, 0x80].to_vec()),
            I2cTransaction::transaction_start(0x2c),
            I2cTransaction::write(0x2c, [0x09].to_vec()),
            I2cTransaction::write(0x2c, [0x01, 0x02, 0x40].to_vec()),
            I2cTransaction::transaction_end(0x2c),
            I2cTransaction::write(0x2C, [0x01, 0x03].to_vec()),
            I2cTransaction::write_read(0x2C, [0x05].to_vec(), [0x00].to_vec()),
            I2cTransaction::write_read(0x2C, [0x05].to_vec(), [0x1B].to_vec()),
            I2cTransaction::write_read(0x2C, [0x05].to_vec(), [0x1F].to_vec()),
            I2cTransaction::write(0x2C, [0x01, 0x00].to_vec()),
            I2cTransaction::write_read(0x2C, [0x22].to_vec(), [0xbe].to_vec()),
            I2cTransaction::write_read(0x2C, [0x21].to_vec(), [0xef].to_vec()),
        ];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).calculate_crc(&[0x01, 0x02, 0x40]),
            Ok([0xbe, 0xef])
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_check_error_register_protocol() {
        let expectations = [I2cTransaction::write_read(
            0x2C,
            [0x06].to_vec(),
            [0x01].to_vec(),
        )];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).check_error_register(),
            Err(crate::error::Error::Protocol)
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_check_error_crc_err() {
        let expectations = [I2cTransaction::write_read(
            0x2C,
            [0x06].to_vec(),
            [0x04].to_vec(),
        )];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).check_error_register(),
            Err(crate::error::Error::Crc)
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_check_error_ok() {
        let expectations = [I2cTransaction::write_read(
            0x2C,
            [0x06].to_vec(),
            [0x00].to_vec(),
        )];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(Mfrc522::new(i2c).check_error_register(), Ok(()));

        i2c_clone.done();
    }

    #[test]
    pub fn test_transceive_rx_0() {
        let expectations = [
            I2cTransaction::write(0x2C, [0x01, 0x00].to_vec()),
            I2cTransaction::write(0x2C, [0x04, 0x7f].to_vec()),
            I2cTransaction::write(0x2C, [0x0A, 0x80].to_vec()),
            I2cTransaction::write(0x2C, [0x09, 0xfe, 0xed].to_vec()),
            I2cTransaction::write(0x2C, [0x01, 0x0C].to_vec()),
            I2cTransaction::write(0x2C, [0x0D, 0xA1].to_vec()),
            I2cTransaction::write_read(0x2C, [0x04].to_vec(), [0x04].to_vec()),
            I2cTransaction::write_read(0x2C, [0x04].to_vec(), [0x02].to_vec()),
            I2cTransaction::write_read(0x2C, [0x06].to_vec(), [0x00].to_vec()),
        ];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c)
            .transceive::<0>(&[0xfe, 0xed], 0xf9, 0xfa)
            .unwrap();

        i2c_clone.done();
    }

    #[test]
    pub fn test_transceive() {
        let expectations = [
            I2cTransaction::write(0x2C, [0x01, 0x00].to_vec()),
            I2cTransaction::write(0x2C, [0x04, 0x7f].to_vec()),
            I2cTransaction::write(0x2C, [0x0A, 0x80].to_vec()),
            I2cTransaction::transaction_start(0x2c),
            I2cTransaction::write(0x2C, [0x09].to_vec()),
            I2cTransaction::write(0x2C, [0xfe, 0xed].to_vec()),
            I2cTransaction::transaction_end(0x2c),
            I2cTransaction::write(0x2C, [0x01, 0x0C].to_vec()),
            I2cTransaction::write(0x2C, [0x0D, 0xA1].to_vec()),
            I2cTransaction::write_read(0x2C, [0x04].to_vec(), [0x04].to_vec()),
            I2cTransaction::write_read(0x2C, [0x04].to_vec(), [0x02].to_vec()),
            I2cTransaction::write_read(0x2C, [0x06].to_vec(), [0x00].to_vec()),
            I2cTransaction::write_read(0x2C, [0x0A].to_vec(), [0x04].to_vec()),
            I2cTransaction::write_read(0x2C, [0x09].to_vec(), [0x98, 0x76, 0x52, 0x2b].to_vec()),
            I2cTransaction::write_read(0x2C, [0x0C].to_vec(), [0x91].to_vec()),
        ];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).transceive::<4>(&[0xfe, 0xed], 0xf9, 0xfa),
            Ok(crate::FifoData {
                buffer: [0x98, 0x76, 0x52, 0x2b],
                valid_bytes: 4,
                valid_bits: 1,
            })
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_fifo_data_0() {
        let i2c = I2cMock::new(&[]);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).fifo_data::<0>(),
            Ok(crate::FifoData {
                buffer: [],
                valid_bytes: 0,
                valid_bits: 0,
            })
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_fifo_data_1() {
        let i2c = I2cMock::new(&[
            I2cTransaction::write_read(0x2C, [0x0A].to_vec(), [0x01].to_vec()),
            I2cTransaction::write_read(0x2C, [0x09].to_vec(), [0x98].to_vec()),
            I2cTransaction::write_read(0x2C, [0x0C].to_vec(), [0x07].to_vec()),
        ]);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).fifo_data::<1>(),
            Ok(crate::FifoData {
                buffer: [0x98],
                valid_bytes: 1,
                valid_bits: 7,
            })
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_fifo_flush() {
        let expectations = [I2cTransaction::write(0x2C, [0x0A, 0x80].to_vec())];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c).fifo_flush().unwrap(); //.init().unwrap();

        i2c_clone.done();
    }

    #[test]
    pub fn test_command() {
        let expectations = [I2cTransaction::write(0x2C, [0x01, 0x0E].to_vec())];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c)
            .command(crate::register::Command::MFAuthent)
            .unwrap(); //.init().unwrap();

        i2c_clone.done();
    }

    #[test]
    pub fn test_reset() {
        let expectations = [
            I2cTransaction::write(0x2C, [0x01, 0x0F].to_vec()),
            I2cTransaction::write_read(0x2C, [0x01].to_vec(), [0x30].to_vec()),
            I2cTransaction::write_read(0x2C, [0x01].to_vec(), [0x10].to_vec()),
            I2cTransaction::write_read(0x2C, [0x01].to_vec(), [0x0f].to_vec()),
        ];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c).reset().unwrap(); //.init().unwrap();

        i2c_clone.done();
    }

    #[test]
    pub fn test_read() {
        let expectations = [I2cTransaction::write_read(
            0x2C,
            [0x0B].to_vec(),
            [0x37].to_vec(),
        )];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        assert_eq!(
            Mfrc522::new(i2c).read(crate::register::Register::WaterLevelReg),
            Ok(0x37)
        );

        i2c_clone.done();
    }

    #[test]
    pub fn test_read_many_2() {
        let expectations = [
            I2cTransaction::write_read(0x2C, [0x15].to_vec(), [0x12].to_vec()),
            I2cTransaction::write_read(0x2C, [0x15].to_vec(), [0x23].to_vec()),
        ];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        let mut buffer = [0u8; 2];
        Mfrc522::new(i2c)
            .read_many(crate::register::Register::TxASKReg, &mut buffer)
            .unwrap();
        assert_eq!(buffer, [0x12, 0x23]);

        i2c_clone.done();
    }

    #[test]
    pub fn test_read_many_3() {
        let expectations = [
            I2cTransaction::write_read(0x2C, [0x15].to_vec(), [0x12].to_vec()),
            I2cTransaction::write_read(0x2C, [0x15].to_vec(), [0x23].to_vec()),
            I2cTransaction::write_read(0x2C, [0x15].to_vec(), [0x34].to_vec()),
        ];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        let mut buffer = [0u8; 3];
        Mfrc522::new(i2c)
            .read_many(crate::register::Register::TxASKReg, &mut buffer)
            .unwrap();
        assert_eq!(buffer, [0x12, 0x23, 0x34]);

        i2c_clone.done();
    }

    #[test]
    pub fn test_rmw() {
        let expectations = [
            I2cTransaction::write_read(0x2C, [0x13].to_vec(), [0x37].to_vec()),
            I2cTransaction::write(0x2C, [0x13, 0xC8].to_vec()),
        ];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c)
            .rmw(crate::register::Register::RxModeReg, |b| !b)
            .unwrap();

        i2c_clone.done();
    }

    #[test]
    pub fn test_write() {
        let expectations = [I2cTransaction::write(0x2C, [0x21, 0xfd].to_vec())];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c)
            .write(crate::register::Register::CRCResultRegHigh, 0xfd)
            .unwrap();

        i2c_clone.done();
    }

    #[test]
    pub fn test_write_many() {
        let expectations = [I2cTransaction::write(
            0x2C,
            [0x27, 0xca, 0xfe, 0xf0, 0x0d].to_vec(),
        )];

        let i2c = I2cMock::new(&expectations);
        let mut i2c_clone = i2c.clone();
        let i2c = I2cInterface::new(i2c, 0x2c);

        Mfrc522::new(i2c)
            .write_many(crate::register::Register::GsNReg, &[0xca, 0xfe, 0xf0, 0x0d])
            .unwrap();

        i2c_clone.done();
    }
}
