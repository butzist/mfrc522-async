/// Error type used in this crate
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, PartialEq)]
pub enum Error<SpiE, GpioE> {
    /// Wrong Block Character Check (BCC)
    Bcc,
    /// FIFO buffer overflow
    BufferOverflow,
    /// Collision
    Collision,
    /// Wrong CRC
    Crc,
    /// Incomplete RX frame
    IncompleteFrame,
    /// Internal temperature sensor detects overheating
    Overheating,
    /// Parity check failed
    Parity,
    /// Error during MFAuthent operation
    Protocol,
    /// Timeout
    Timeout,
    /// Write error: FIFO buffer was written at invalid time
    Wr,
    /// Not acknowledge
    Nak,
    /// Provided buffer not large enough
    NoRoom,
    /// Proprietary frames, commands or protocols used
    Proprietary,
    /// GPIO wait error
    Gpio(GpioE),
    /// Communication error on the underlying interface
    Comm(SpiE),
}
