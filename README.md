# Async MFRC522 Driver with IRQ Support

This is an async implementation of an MFRC522 RFID reader driver with interrupt
(IRQ) support for retrieving PICC UIDs. The driver is designed for `no_std`
environments using `embedded-hal-async` traits.

It was forked from the blocking driver
[jspngh/mfrc522](https://gitlab.com/jspngh/mfrc522) and is available under the
same license (MIT/Apache, see below).

## Features

- **Async API**: Uses `embedded-hal-async` for non-blocking operations
- **IRQ Support**: Interrupt-driven card detection using MFRC522 IRQ pin
- **UID Retrieval**: Complete REQA → Anticollision → Select sequence to read
  PICC UIDs
- **ISO 14443A compliant**: Follows the standard PICC communication protocol
- **`no_std` compatible**: Suitable for embedded systems

## Current Implementation Status

### ✅ Implemented

- **REQA command** (`reqa()`): Request Type A command to detect PICCs
- **SELECT command** (`select()`): Complete anticollision and selection sequence
- **IRQ-based communication**: Uses MFRC522 IRQ pin for async operation
- **UID extraction**: Supports Single (4-byte), Double (7-byte), and Triple
  (10-byte) UIDs
- **Basic error handling**: CRC, collision, timeout, and protocol errors
- **CRC calculation**: Hardware-accelerated CRC generation and verification

## API Usage

```rust
use mfrc522_async::{Mfrc522, Error};
use embedded_hal_async::{digital::Wait, spi::SpiDevice};

// Create driver instance
let mut mfrc522 = Mfrc522::new(spi_device, irq_pin, enable_pin);

// Power up and initialize the MFRC522
let mfrc522 = mfrc522.enable().unwrap();
let mfrc522 = mfrc522.init().await?;

// Request card presence (interrupt-driven)
let atqa = mfrc522.reqa().await?;

// Select the card and retrieve UID
let uid = mfrc522.select(&atqa).await?;

match uid {
    Uid::Single(uid) => println!("UID: {:02X?}", uid.as_bytes()),
    Uid::Double(uid) => println!("UID: {:02X?}", uid.as_bytes()),
    Uid::Triple(uid) => println!("UID: {:02X?}", uid.as_bytes()),
}
```

## IRQ-Based Operation

The MFRC522 does not automatically generate interrupts on card presence.
Instead:

1. The driver enables interrupt sources (RX_IRQ, IDLE_IRQ, ERR_IRQ) in the
   MFRC522
2. When `reqa()` is called, it sends the REQA command and waits for the IRQ pin
3. The IRQ pin triggers when the MFRC522 receives the ATQA response from a PICC
4. The driver then reads the FIFO data to get the ATQA and proceeds with
   selection

This approach aligns with the MFRC522 datasheet and common implementation
patterns.

## Hardware Requirements

- **SPI interface**: MOSI, MISO, SCK, and SS pins
- **IRQ pin**: GPIO pin capable of interrupt handling (Wait trait)
- **RST pin**: Not needed
- **Power**: 3.3V power supply

## Error Handling

The driver provides comprehensive error handling for:

- Communication errors (SPI/I2C)
- GPIO errors
- Timeout errors
- CRC errors
- Collision errors
- Protocol errors
- Buffer overflow/underflow
- Overheating warnings

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

This is a work-in-progress implementation. Contributions welcome for:

- Adding comprehensive async tests
- Adding additional MFRC522 features (authentication, read/write operations)
- Documentation improvements
- Examples

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
