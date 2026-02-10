//! Version test for MFRC522 driver - demonstrates actual async API usage

use crate::Mfrc522;
use embedded_hal_mock::eh1::digital::Mock as PinMock;
use embedded_hal_mock::eh1::spi::{Mock as SpiMock, Transaction as SpiTransaction};

#[tokio::test]
async fn test_version() {
    // Mock the version register read
    let expectations = [
        // read_register: VersionReg (0x37)
        SpiTransaction::transaction_start(),
        SpiTransaction::transfer_in_place([0xEE, 0x00].to_vec(), [0xEE, 0x91].to_vec()), // VersionReg read
        SpiTransaction::transaction_end(),
    ];

    let spi = SpiMock::new(&expectations);
    let mut spi_clone = spi.clone();
    let irq = PinMock::new(&[]);
    let mut irq_clone = irq.clone();

    // Create MFRC522 and directly test version() which doesn't require init
    let mut mfrc522 = Mfrc522::new(spi, irq);

    // Test the version() method
    let version = mfrc522.version().await.unwrap();
    assert_eq!(version, 0x91); // Expected version for MFRC522

    // Verify done is called on mocks
    spi_clone.done();
    irq_clone.done();
}
