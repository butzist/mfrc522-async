mod constructor_tests;
mod crc;
mod init;
mod reset_to_idle;
mod transceive;
mod version;

extern crate alloc;

use core::iter::{once, repeat_n};

use alloc::vec::Vec;

use embedded_hal_mock::eh1::spi::Transaction as SpiTransaction;

// reset_to_idle() sequence:
// 1. clear_irq_state(): writes 0x7F to ComIrqReg and DivIrqReg, then waits for IRQ low
// 2. command(Idle): writes 0x00 to CommandReg
// 3. fifo_flush(): writes 0x80 (FLUSH_BUFFER) to FIFOLevelReg
fn add_reset_to_idle_sequence(expectations: &mut Vec<SpiTransaction<u8>>) {
    expectations.extend([
        // clear_irq_state(): ComIrqReg address = 0x04 << 1 = 0x08, value = 0x7F
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x08, 0x7F].to_vec()),
        SpiTransaction::transaction_end(),
        // clear_irq_state(): DivIrqReg address = 0x05 << 1 = 0x0A, value = 0x7F
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x0A, 0x7F].to_vec()),
        SpiTransaction::transaction_end(),
        // command(Idle): CommandReg address = 0x01 << 1 = 0x02, value = 0x00
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x02, 0x00].to_vec()),
        SpiTransaction::transaction_end(),
        // fifo_flush(): FIFOLevelReg address = 0x0A << 1 = 0x14, value = 0x80
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec([0x14, 0x80].to_vec()),
        SpiTransaction::transaction_end(),
    ]);
}

fn add_fifo_data_sequence(expectations: &mut Vec<SpiTransaction<u8>>, content: &[u8]) {
    let request: Vec<u8> = repeat_n(0x92, content.len() - 1)
        .chain(once(0x00))
        .collect();
    expectations.extend([
        // read FifoLevelReg
        SpiTransaction::transaction_start(),
        SpiTransaction::transfer_in_place(
            [0x94, 0x00].to_vec(),
            [0xe0, content.len() as u8].to_vec(),
        ),
        SpiTransaction::transaction_end(),
        SpiTransaction::transaction_start(),
        // read 4 bytes from FifoDataReg
        SpiTransaction::write_vec([0x92].to_vec()),
        SpiTransaction::transfer_in_place(request, content.to_vec()),
        SpiTransaction::transaction_end(),
        // read ControlReg
        SpiTransaction::transaction_start(),
        SpiTransaction::transfer_in_place([0x98, 0x00].to_vec(), [0x30, 0xf3].to_vec()),
        SpiTransaction::transaction_end(),
    ]);
}

fn add_wait_for_irq_source_sequence(
    expectations: &mut Vec<SpiTransaction<u8>>,
    div_irq_reg: u8,
    com_irq_reg: u8,
) {
    expectations.extend([
        // read DivIrqReg
        SpiTransaction::transaction_start(),
        SpiTransaction::transfer_in_place([0x8A, 0x00].to_vec(), [0x39, div_irq_reg].to_vec()),
        SpiTransaction::transaction_end(),
        // read ComIrqReg
        SpiTransaction::transaction_start(),
        SpiTransaction::transfer_in_place([0x88, 0x00].to_vec(), [0x71, com_irq_reg].to_vec()),
        SpiTransaction::transaction_end(),
    ]);
}
