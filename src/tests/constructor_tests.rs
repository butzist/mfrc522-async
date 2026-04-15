//! Constructor tests for MFRC522 driver

use crate::{Disabled, Mfrc522, Uninitialized, Unknown};
use embedded_hal_mock::eh1::digital::{Mock as PinMock, State, Transaction as PinTransaction};
use embedded_hal_mock::eh1::spi::Mock as SpiMock;

#[tokio::test]
async fn test_new_with_empty_expectations() {
    let mut spi = SpiMock::new(&[]);
    let mut irq = PinMock::new(&[]);
    let mut enable = PinMock::new(&[]);

    let mfrc522 = Mfrc522::new(&mut spi, &mut irq, &mut enable);

    let _mfrc522: Mfrc522<&mut SpiMock<u8>, &mut PinMock, &mut PinMock, Unknown> = mfrc522;

    spi.done();
    irq.done();
    enable.done();
}

#[tokio::test]
async fn test_enable_success() {
    let mut spi = SpiMock::new(&[]);
    let mut irq = PinMock::new(&[]);
    let enable_expectations = [PinTransaction::set(State::High)];
    let mut enable = PinMock::new(&enable_expectations);

    let mfrc522 = Mfrc522::new(&mut spi, &mut irq, &mut enable);
    let result = mfrc522.enable();

    assert!(result.is_ok());
    let _mfrc522: Mfrc522<&mut SpiMock<u8>, &mut PinMock, &mut PinMock, Uninitialized> =
        result.unwrap();

    spi.done();
    irq.done();
    enable.done();
}

#[tokio::test]
async fn test_enable_disable_flow() {
    let mut spi = SpiMock::new(&[]);
    let mut irq = PinMock::new(&[]);

    // Enable and disable expectations
    let enable_expectations = [
        PinTransaction::set(State::High), // enable()
        PinTransaction::set(State::Low),  // disable()
    ];
    let mut enable = PinMock::new(&enable_expectations);

    // Create in Unknown state
    let device = Mfrc522::new(&mut spi, &mut irq, &mut enable);

    // Enable to Uninitialized state
    let device = device.enable().unwrap();

    // Disable back to Disabled state
    let device = device.disable().unwrap();
    let _device: Mfrc522<&mut SpiMock<u8>, &mut PinMock, &mut PinMock, Disabled> = device;

    // Release from Disabled state
    let (_spi, _irq, _enable) = _device.release();

    spi.done();
    irq.done();
    enable.done();
}
