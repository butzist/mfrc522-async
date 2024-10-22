#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;
use rp_pico as bsp;

use bsp::entry;
use embedded_hal::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;
use fugit::RateExtU32;
use mfrc522::{comm::blocking::spi::SpiInterface, Mfrc522, Uid};

use bsp::hal::{
    clocks::{init_clocks_and_plls, Clock},
    gpio::{FunctionSpi, Pin, PullNone, PullUp},
    pac,
    sio::Sio,
    spi,
    watchdog::Watchdog,
    Timer,
};

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    // External high-speed crystal on the pico board is 12Mhz
    let external_xtal_freq_hz = 12_000_000u32;
    let clocks = init_clocks_and_plls(
        external_xtal_freq_hz,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let mut delay = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // These are implicitly used by the spi driver if they are in the correct mode
    let spi_sclk: Pin<_, FunctionSpi, PullNone> = pins.gpio2.reconfigure();
    let spi_mosi: Pin<_, FunctionSpi, PullNone> = pins.gpio3.reconfigure();
    let spi_miso: Pin<_, FunctionSpi, PullUp> = pins.gpio4.reconfigure();
    let spi_cs = pins.gpio5.into_push_pull_output();

    let spi_pin_layout = (spi_mosi, spi_miso, spi_sclk);

    // Create an SPI driver instance for the SPI0 device
    let spi = spi::Spi::<_, _, _, 8>::new(pac.SPI0, spi_pin_layout);

    // Exchange the uninitialised SPI driver for an initialised one
    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        1_000_000u32.Hz(),
        embedded_hal::spi::MODE_0,
    );

    let spi = ExclusiveDevice::new(spi, spi_cs, delay).unwrap();
    let itf = SpiInterface::new(spi);
    let mut mfrc522 = Mfrc522::new(itf).init().unwrap();
    match mfrc522.version() {
        Ok(version) => defmt::info!("version {:x}", version),
        Err(_e) => defmt::error!("version error"),
    }

    loop {
        if let Ok(atqa) = mfrc522.wupa() {
            defmt::info!("new card detected");
            match mfrc522.select(&atqa) {
                Ok(ref _uid @ Uid::Single(ref inner)) => {
                    defmt::info!("card uid {=[?]}", inner.as_bytes());
                }
                Ok(ref _uid @ Uid::Double(ref inner)) => {
                    defmt::info!("card double uid {=[?]}", inner.as_bytes());
                }
                Ok(_) => defmt::info!("got other uid size"),
                Err(_e) => {
                    defmt::error!("Select error");
                }
            }
        }
        delay.delay_ms(1000u32);
    }
}
