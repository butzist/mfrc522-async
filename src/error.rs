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

/// Error type returned when enable or disable operations fail.
///
/// On failure, this type provides methods to recover the underlying resources
/// or access the GPIO error that caused the failure.
pub struct EnableError<SPI, IRQ, EN>
where
    EN: embedded_hal::digital::OutputPin,
{
    pub(crate) device: crate::Mfrc522<SPI, IRQ, EN, crate::Unknown>,
    pub(crate) error: EN::Error,
}

impl<SPI, IRQ, EN> core::fmt::Debug for EnableError<SPI, IRQ, EN>
where
    EN: embedded_hal::digital::OutputPin,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.error.fmt(f)
    }
}

impl<SPI, IRQ, EN> EnableError<SPI, IRQ, EN>
where
    EN: embedded_hal::digital::OutputPin,
{
    /// Create a new EnableError with the device and error.
    pub fn new(error: EN::Error, device: crate::Mfrc522<SPI, IRQ, EN, crate::Unknown>) -> Self {
        Self { device, error }
    }

    /// Consume the error and return the device.
    ///
    /// This allows you to recover the device in the `Unknown` state
    /// if enable/disable failed.
    pub fn into_device(self) -> crate::Mfrc522<SPI, IRQ, EN, crate::Unknown> {
        self.device
    }

    /// Get a reference to the underlying GPIO error.
    pub fn error(&self) -> &EN::Error {
        &self.error
    }

    /// Consume the error and return the underlying GPIO error.
    ///
    /// If you don't need to recover the resources, this lets you
    /// access just the error that caused the enable/disable to fail.
    pub fn into_error(self) -> EN::Error {
        self.error
    }
}

/// Error type returned when initialization fails.
///
/// On failure, this type provides methods to recover the underlying resources
/// or access the error that caused the failure.
pub struct InitError<SPI, IRQ, EN, E>
where
    EN: embedded_hal::digital::OutputPin,
{
    pub(crate) device: crate::Mfrc522<SPI, IRQ, EN, crate::Uninitialized>,
    pub(crate) error: E,
}

impl<SPI, IRQ, EN, E> core::fmt::Debug for InitError<SPI, IRQ, EN, E>
where
    EN: embedded_hal::digital::OutputPin,
    E: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.error.fmt(f)
    }
}

impl<SPI, IRQ, EN, E> InitError<SPI, IRQ, EN, E>
where
    EN: embedded_hal::digital::OutputPin,
{
    /// Create a new InitError with the device and error.
    pub fn new(error: E, device: crate::Mfrc522<SPI, IRQ, EN, crate::Uninitialized>) -> Self {
        Self { device, error }
    }

    /// Consume the error and return the device.
    ///
    /// This allows you to recover the device in the `Uninitialized` state
    /// if initialization failed.
    pub fn into_device(self) -> crate::Mfrc522<SPI, IRQ, EN, crate::Uninitialized> {
        self.device
    }

    /// Get a reference to the underlying error.
    pub fn error(&self) -> &E {
        &self.error
    }

    /// Consume the error and return the underlying error.
    ///
    /// If you don't need to recover the resources, this lets you
    /// access just the error that caused the initialization to fail.
    pub fn into_error(self) -> E {
        self.error
    }
}
