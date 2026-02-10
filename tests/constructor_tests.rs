//! Constructor tests for MFRC522 driver

use embedded_hal_mock::eh1::digital::Mock as PinMock;
use embedded_hal_mock::eh1::spi::Mock as SpiMock;
use mfrc522_async::{Mfrc522, Uninitialized};

#[tokio::test]
async fn test_new_with_empty_expectations() {
    let mut spi = SpiMock::new(&[]);
    let mut irq = PinMock::new(&[]);
    let mfrc522 = Mfrc522::new(&mut spi, &mut irq);

    // Just verify that the constructor creates a valid instance
    // The type system will verify this compiles
    let _mfrc522: Mfrc522<&mut SpiMock<u8>, &mut PinMock, Uninitialized> = mfrc522;

    spi.done();
    irq.done();
}
