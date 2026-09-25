#![no_std]
#![no_main]

use defmt::info;
use defmt_rtt as _;
use embassy_nrf::nvmc::Nvmc;
use embedded_storage::nor_flash::NorFlash;
use nrf_mpsl as _;
use panic_probe as _;

#[cortex_m_rt::entry]
fn main() -> ! {
    info!("Settings Reset Utility starting...");

    let p = embassy_nrf::init(Default::default());
    let mut nvmc = Nvmc::new(p.NVMC);

    // Erase storage partition (0xA0000..0xB8000, 24 sectors = 96KB to cover all storage/bonding areas)
    info!("Erasing storage and bonding flash sectors from 0xA0000 to 0xB8000...");
    let start_addr: u32 = 0xA0000;
    let end_addr: u32 = 0xB8000;

    let res = nvmc.erase(start_addr, end_addr);
    match res {
        Ok(_) => info!("Storage sectors erased successfully!"),
        Err(_) => info!("Failed to erase storage sectors."),
    }

    info!("Rebooting into bootloader/system in 1 second...");
    cortex_m::asm::delay(64_000_000);

    // Request system reset
    cortex_m::peripheral::SCB::sys_reset();
}
