//! Reset to idle tests for MFRC522 driver

use crate::Mfrc522;
use crate::tests::add_reset_to_idle_sequence;
use embedded_hal_mock::eh1::digital::{
    Mock as PinMock, State, Transaction as PinTransaction, TransactionKind,
};
use embedded_hal_mock::eh1::spi::Mock as SpiMock;

#[tokio::test]
#[allow(unsafe_code)]
async fn test_reset_to_idle() {
    let mut expectations = Default::default();
    add_reset_to_idle_sequence(&mut expectations);

    let mut spi = SpiMock::new(&expectations);

    // Mock the wait_for_low() call from clear_irq_state()
    let irq_expectations = [PinTransaction::new(TransactionKind::WaitForState(
        State::Low,
    ))];
    let mut irq = PinMock::new(&irq_expectations);

    // Enable pin - empty expectations since we're not testing enable/disable
    let mut enable = PinMock::new(&[]);

    // Create initialized MFRC522 for testing
    let mut mfrc522 = unsafe { Mfrc522::new_initialized(&mut spi, &mut irq, &mut enable) };

    // Call reset_to_idle() - this should match the expectations above
    let result = mfrc522.reset_to_idle().await;

    assert_eq!(result, Ok(()));

    spi.done();
    irq.done();
    enable.done();
}
