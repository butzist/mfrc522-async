//! Blocking implementation of the `Interface` trait for I2C communication.

use crate::comm::Interface;
use crate::register::Register;

use embedded_hal_02::blocking::i2c;
use heapless::Vec;

/// Blocking I2C interface to the MFRC522
pub struct I2cInterface<I2C> {
    /// The actual I2C device
    i2c: I2C,
    /// The bus address of the MFRC522
    addr: u8,
}

impl<E, I2C> I2cInterface<I2C>
where
    I2C: i2c::Write<Error = E> + i2c::WriteRead<Error = E>,
{
    /// Create a new I2C interface.
    pub fn new(i2c: I2C, addr: u8) -> Self {
        Self {
            i2c,
            addr,
        }
    }
}

impl<I2C> I2cInterface<I2C> {
    /// Release the underlying I2C device
    pub fn release(self) -> I2C {
        self.i2c
    }
}

impl<E, I2C> Interface for I2cInterface<I2C>
where
    I2C: i2c::Write<Error = E> + i2c::WriteRead<Error = E>,
{
    type Error = E;

    fn read(&mut self, reg: Register) -> Result<u8, Self::Error> {
        let mut buffer = [0];
        self.i2c.write_read(self.addr, &[reg as u8], &mut buffer)?;
        Ok(buffer[0])
    }

    fn read_many<'b>(&mut self, reg: Register, buf: &'b mut [u8]) -> Result<&'b [u8], Self::Error> {
        self.i2c.write_read(self.addr, &[reg as u8], buf)?;
        Ok(buf)
    }

    fn write(&mut self, reg: Register, val: u8) -> Result<(), Self::Error> {
        self.i2c.write(self.addr, &[reg as u8, val])?;
        Ok(())
    }

    fn write_many(&mut self, reg: Register, bytes: &[u8]) -> Result<(), Self::Error> {
        let mut vec = Vec::<u8, 65>::new();
        vec.push(reg as u8).unwrap();
        vec.extend_from_slice(bytes).unwrap();
        self.i2c.write(self.addr, vec.as_slice())?;
        Ok(())
    }
}

