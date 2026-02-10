//! CRC calculation tests for MFRC522 driver

use crate::Mfrc522;
use crate::tests::{add_reset_to_idle_sequence, add_wait_for_irq_source_sequence};

use embedded_hal_mock::eh1::digital::{Mock as PinMock, Transaction as PinTransaction};
use embedded_hal_mock::eh1::spi::{Mock as SpiMock, Transaction as SpiTransaction};

#[tokio::test]
#[allow(unsafe_code)]
async fn test_calculate_crc() {
    // Based on original crc test but adapted for the async implementation with IRQ handling
    let mut expectations = Default::default();
    add_reset_to_idle_sequence(&mut expectations);

    expectations.extend([
        // Write data to FIFO
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x12].to_vec()), // FIFODataReg address
        SpiTransaction::write_vec([0x01, 0x02, 0x40].to_vec()), // data
        SpiTransaction::transaction_end(),
        // Start CRC calculation
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x02, 0x03].to_vec()), // CalcCRC command
        SpiTransaction::transaction_end(),
    ]);
    add_wait_for_irq_source_sequence(&mut expectations, 0x04, 0x00);

    // Read CRC result via read_many (transaction)
    expectations.extend([
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x02, 0x00].to_vec()), // Set to IDLE
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::transfer_in_place([0xC4, 0x00].to_vec(), [0x29, 0xbe].to_vec()),
        // CRCResultLow
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::transfer_in_place([0xC2, 0x00].to_vec(), [0x93, 0xef].to_vec()),
        // CRCResultHigh
        SpiTransaction::transaction_end(),
    ]);

    let pin_expectations = [
        PinTransaction::wait_for_state(embedded_hal_mock::eh1::digital::State::Low),
        PinTransaction::wait_for_state(embedded_hal_mock::eh1::digital::State::High),
    ];

    let spi = SpiMock::new(&expectations);
    let mut spi_clone = spi.clone();
    let irq = PinMock::new(&pin_expectations);
    let mut irq_clone = irq.clone();

    // Create initialized MFRC522 for testing
    let mut mfrc522 = unsafe { Mfrc522::new_initialized(spi, irq) };

    let result = mfrc522.calculate_crc(&[0x01, 0x02, 0x40]).await;

    assert_eq!(result, Ok([0xbe, 0xef]));

    spi_clone.done();
    irq_clone.done();
}
