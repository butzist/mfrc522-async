#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::spi::{Config, Spi};
use embassy_stm32::time::Hertz;
use embassy_time::{Delay, Duration, Timer};

use embedded_hal_bus::spi::ExclusiveDevice;

use mfrc522::comm::{blocking::spi::SpiInterface, Interface};
use mfrc522::{Error, Initialized, Mfrc522, Uid};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    let mut spi_config = Config::default();
    spi_config.frequency = Hertz(1_000_000);
    let spi = Spi::new_blocking(p.SPI1, p.PA5, p.PA7, p.PA6, spi_config);

    let cs = Output::new(p.PB6, Level::High, Speed::VeryHigh);

    let spi = ExclusiveDevice::new(spi, cs, Delay);
    let itf = SpiInterface::new(spi);
    let mut mfrc522 = Mfrc522::new(itf).init().expect("could not create MFRC522");

    match mfrc522.version() {
        Ok(version) => defmt::info!("version {:x}", version),
        Err(_e) => defmt::error!("version error"),
    }

    let write = false;
    loop {
        if let Ok(atqa) = mfrc522.wupa() {
            defmt::info!("new card detected");
            match mfrc522.select(&atqa) {
                Ok(ref uid @ Uid::Single(ref inner)) => {
                    defmt::info!("card uid {=[?]}", inner.as_bytes());
                    handle_card(&mut mfrc522, uid, write);
                }
                Ok(ref uid @ Uid::Double(ref inner)) => {
                    defmt::info!("card double uid {=[?]}", inner.as_bytes());
                    handle_card(&mut mfrc522, uid, write);
                }
                Ok(_) => defmt::info!("got other uid size"),
                Err(e) => {
                    defmt::error!("Select error");
                    print_err(&e);
                }
            }
        }
        Timer::after(Duration::from_millis(1000)).await;
    }
}

fn handle_card<E, COMM: Interface<Error = E>>(
    mfrc522: &mut Mfrc522<COMM, Initialized>,
    uid: &Uid,
    write: bool,
) {
    let key = [0xFF; 6];
    #[rustfmt::skip]
    let buffer = [
        0xDE, 0x42, 0xAD, 0x42, 0xBE, 0x42, 0xEF, 0x42,
        0xCA, 0x42, 0xFE, 0x42, 0xBA, 0x42, 0xBE, 0x42,
    ];
    if mfrc522.mf_authenticate(uid, 1, &key).is_ok() {
        if write {
            match mfrc522.mf_write(1, buffer) {
                Ok(_) => {
                    defmt::info!("write success");
                }
                Err(e) => {
                    defmt::error!("error during read");
                    print_err(&e);
                }
            }
        } else {
            match mfrc522.mf_read(1) {
                Ok(data) => defmt::info!("read {=[?]}", data),
                Err(e) => {
                    defmt::error!("error during read");
                    print_err(&e);
                }
            }
        }
    } else {
        defmt::warn!("Could not authenticate");
    }

    if mfrc522.hlta().is_err() {
        defmt::error!("Could not halt");
    }
    if mfrc522.stop_crypto1().is_err() {
        defmt::error!("Could not disable crypto1");
    }
}

fn print_err<E>(err: &Error<E>) {
    match err {
        Error::Bcc => defmt::error!("error BCC"),
        Error::BufferOverflow => defmt::error!("error BufferOverflow"),
        Error::Collision => defmt::error!("error Collision"),
        Error::Crc => defmt::error!("error Crc"),
        Error::IncompleteFrame => defmt::error!("error Incomplete"),
        Error::NoRoom => defmt::error!("error NoRoom"),
        Error::Overheating => defmt::error!("error Overheating"),
        Error::Parity => defmt::error!("error Parity"),
        Error::Protocol => defmt::error!("error Protocol"),
        Error::Timeout => defmt::error!("error Timeout"),
        Error::Wr => defmt::error!("error Wr"),
        Error::Nak => defmt::error!("error Nak"),
        _ => defmt::error!("error SPI"),
    };
}
