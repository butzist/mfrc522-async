//! Transceive tests for MFRC522 driver

use crate::Mfrc522;
use crate::tests::{
    add_fifo_data_sequence, add_reset_to_idle_sequence, add_wait_for_irq_source_sequence,
};
use embedded_hal_mock::eh1::digital::{Mock as PinMock, Transaction as PinTransaction};
use embedded_hal_mock::eh1::spi::{Mock as SpiMock, Transaction as SpiTransaction};

#[tokio::test]
#[allow(unsafe_code)]
async fn test_transceive() {
    // Based on original transceive test from tests-old.rs
    let mut expectations = Default::default();
    add_reset_to_idle_sequence(&mut expectations);
    expectations.extend(
        [
            // Write data to FIFO
            SpiTransaction::transaction_start(),
            SpiTransaction::write_vec([0x12].to_vec()),
            SpiTransaction::write_vec([0xfe, 0xed].to_vec()),
            SpiTransaction::transaction_end(),
            // Start transceive command
            SpiTransaction::transaction_start(),
            SpiTransaction::write_vec([0x02, 0x0C].to_vec()),
            SpiTransaction::transaction_end(),
            // Configure bit framing register for send/receive
            SpiTransaction::transaction_start(),
            SpiTransaction::write_vec([0x1A, 0xA1].to_vec()),
            SpiTransaction::transaction_end(),
        ]
        .into_iter(),
    );

    // Set IRQ_RX
    add_wait_for_irq_source_sequence(&mut expectations, 0x00, 0x20);
    // Store received data in FIFO
    add_fifo_data_sequence(&mut expectations, &[0x98, 0x76, 0x52, 0x2b]);

    let pin_expectations = [
        PinTransaction::wait_for_state(embedded_hal_mock::eh1::digital::State::Low),
        PinTransaction::wait_for_state(embedded_hal_mock::eh1::digital::State::High),
    ];

    let mut spi = SpiMock::new(&expectations);
    let mut irq = PinMock::new(&pin_expectations);

    // Enable pin - empty expectations since we're not testing enable/disable
    let mut enable = PinMock::new(&[]);

    // Create initialized MFRC522 for testing
    let mut mfrc522 = unsafe { Mfrc522::new_initialized(&mut spi, &mut irq, &mut enable) };

    let result = mfrc522.transceive::<4>(&[0xfe, 0xed], 0xf9, 0xfa).await;

    assert_eq!(
        result,
        Ok(crate::FifoData {
            buffer: [0x98, 0x76, 0x52, 0x2b],
            valid_bytes: 4,
            valid_bits: 3,
        })
    );

    spi.done();
    irq.done();
    enable.done();
}
