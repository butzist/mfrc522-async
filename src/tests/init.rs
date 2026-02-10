//! Initialization tests for MFRC522 driver

use crate::Mfrc522;
use embedded_hal_mock::eh1::digital::Mock as PinMock;
use embedded_hal_mock::eh1::spi::{Mock as SpiMock, Transaction as SpiTransaction};

#[tokio::test]
async fn test_init_full_sequence_expectations() {
    let expectations = [
        // reset(): command SoftReset - CommandReg (0x01) -> 0x0F
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x02, 0x0F].to_vec()),
        SpiTransaction::transaction_end(),
        // reset(): read CommandReg until POWER_DOWN bit clears (2 reads)
        SpiTransaction::transaction_start(),
        SpiTransaction::transfer_in_place([0x82, 0x00].to_vec(), [0x82, 0x10].to_vec()), // still in power down
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::transfer_in_place([0x82, 0x00].to_vec(), [0x82, 0x00].to_vec()), // power down cleared
        SpiTransaction::transaction_end(),
        // init(): register writes (11 writes)
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x24, 0x00].to_vec()), // TxModeReg = 0x00
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x26, 0x00].to_vec()), // RxModeReg = 0x00
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x48, 0x26].to_vec()), // ModWidthReg = 0x26
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x54, 0x80].to_vec()), // TModeReg = 0x80
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x56, 0xA9].to_vec()), // TPrescalerReg = 0xA9
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x58, 0x03].to_vec()), // TReloadRegHigh = 0x03
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x5A, 0xE8].to_vec()), // TReloadRegLow = 0xE8
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x2A, 0x40].to_vec()), // TxASKReg = 0x40
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x22, 0x3D].to_vec()), // ModeReg = 0x3D
        SpiTransaction::transaction_end(),
        // init(): modify TxControlReg (read + write = 2 transactions)
        SpiTransaction::transaction_start(),
        SpiTransaction::transfer_in_place([0xA8, 0x00].to_vec(), [0xA8, 0x00].to_vec()), // read current
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x28, 0x03].to_vec()), // write with antenna enabled
        SpiTransaction::transaction_end(),
        // init(): interrupt register writes (4 writes)
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x04, 0x33].to_vec()), // ComlEnReg = 0x33
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x06, 0x04].to_vec()), // DivlEnReg = 0x04
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x08, 0x7F].to_vec()), // ComIrqReg = 0x7F (again)
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x0A, 0x7F].to_vec()), // DivIrqReg = 0x7F (again)
        SpiTransaction::transaction_end(),
    ];

    // Create mocks
    let mut spi = SpiMock::new(&expectations);
    let mut irq = PinMock::new(&[]);

    // Create MFRC522 and initialize
    let _mfrc522 = Mfrc522::new(spi.clone(), irq.clone()).init().await.unwrap();

    // Verify done is called on mocks
    spi.done();
    irq.done();
}
