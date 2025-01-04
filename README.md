# Rust MFRC522 driver

This is a `no_std` driver for the MFRC522, an *ISO/IEC 14443 A/MIFARE* reader/writer IC.

This repository is an extension of [japaric/mfrc522](https://github.com/japaric/mfrc522)
and the new home of the `mfrc522` crate.<br>
For more information on the background of this repository, [go here](doc/more_info.md).

What works:
- [x] SPI communication with the MFRC522
- [x] I2C ommunication with the MFRC522
- [ ] UART ommunication with the MFRC522
- [x] Anticollision loop
- [x] Select for 4-byte and 7-byte UIDs
- [x] Mifare Classic authentication
- [x] Reading/writing data
- [ ] Configurable timeout
- [ ] Non-blocking API + support for the interrupt pin
- [ ] Mifare Ultralight C 3DES authentication (no support planned)

## Examples

In the `examples/` directory, you can find examples for:
- STM32L4 (`no_std` + `embassy`)
- Raspberry Pi 2040 (`no_std` + `rp-hal`)
- Raspberry Pi 4 (`std`)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
