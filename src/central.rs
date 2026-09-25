#![no_std]
#![no_main]

#[macro_use]
mod macros;
mod keymap;
mod pointing_processor_controller;

use defmt::{info, unwrap};
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Flex, Input, Level, Output, OutputDrive, Pull};
use embassy_nrf::interrupt::{self, InterruptExt};
use embassy_nrf::mode::Async;
use embassy_nrf::peripherals::{RNG, SAADC, USBD};
use embassy_nrf::saadc::{self, AnyInput, Input as _, Saadc};
use embassy_nrf::usb::Driver;
use embassy_nrf::usb::vbus_detect::HardwareVbusDetect;
use embassy_nrf::{Peri, bind_interrupts, rng, usb};
use nrf_mpsl::Flash;
use nrf_sdc::mpsl::MultiprotocolServiceLayer;
use nrf_sdc::{self as sdc, mpsl};
use panic_probe as _;
use pointing_processor_controller::PointingProcessorController;
use rmk::ble::BleTransport;
use rmk::config::{
    AutoMouseLayerConfig, BehaviorConfig, BleBatteryConfig, DeviceConfig, PositionalConfig, RmkConfig, StorageConfig,
};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::host::HostService;
use rmk::input_device::adc::{AnalogEventType, NrfAdc};
use rmk::input_device::battery::BatteryProcessor;
use rmk::input_device::pmw3610::{BitBangSpiBus, Pmw3610, Pmw3610Config};
use rmk::input_device::pointing::{PointingDevice, PointingProcessor, PointingProcessorConfig};
use rmk::AutoMouseLayerRunner;
use rmk::keyboard::Keyboard;
use rmk::matrix::Matrix;
use rmk::processor::builtin::wpm::WpmProcessor;
use rmk::split::PeripheralMatrixConfig;
use rmk::usb::UsbTransport;
use rmk::watchdog::Nrf52Watchdog;
use rmk::{KeymapData, initialize_keymap_and_storage, run_all};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<USBD>;
    SAADC => saadc::InterruptHandler;
    RNG => rng::InterruptHandler<RNG>;
    EGU0_SWI0 => nrf_sdc::mpsl::LowPrioInterruptHandler;
    CLOCK_POWER => nrf_sdc::mpsl::ClockInterruptHandler, usb::vbus_detect::InterruptHandler;
    RADIO => nrf_sdc::mpsl::HighPrioInterruptHandler;
    TIMER0 => nrf_sdc::mpsl::HighPrioInterruptHandler;
    RTC0 => nrf_sdc::mpsl::HighPrioInterruptHandler;
});

#[embassy_executor::task]
async fn mpsl_task(mpsl: &'static MultiprotocolServiceLayer<'static>) -> ! {
    mpsl.run().await
}

const L2CAP_TXQ: u8 = 3;
const L2CAP_RXQ: u8 = 3;
const L2CAP_MTU: usize = 251;

fn build_sdc<'d, const N: usize>(
    p: nrf_sdc::Peripherals<'d>,
    rng: &'d mut rng::Rng<Async>,
    mpsl: &'d MultiprotocolServiceLayer,
    mem: &'d mut sdc::Mem<N>,
) -> Result<nrf_sdc::SoftdeviceController<'d>, nrf_sdc::Error> {
    sdc::Builder::new()?
        .support_scan()
        .support_central()
        .support_adv()
        .support_peripheral()
        .support_dle_peripheral()
        .support_dle_central()
        .support_phy_update_central()
        .support_phy_update_peripheral()
        .support_le_2m_phy()
        // 1 split peripheral (Left half)
        .central_count(1)?
        // 1 link toward dongle or host
        .peripheral_count(1)?
        .buffer_cfg(L2CAP_MTU as u16, L2CAP_MTU as u16, L2CAP_TXQ, L2CAP_RXQ)?
        .build(p, rng, mpsl, mem)
}

fn init_adc(adc_pin: AnyInput, adc: Peri<'static, SAADC>) -> Saadc<'static, 1> {
    let config = saadc::Config::default();
    let channel_cfg = saadc::ChannelConfig::single_ended(adc_pin.degrade_saadc());
    interrupt::SAADC.set_priority(interrupt::Priority::P3);
    saadc::Saadc::new(adc, Irqs, config, [channel_cfg])
}

fn ble_addr() -> [u8; 6] {
    let ficr = embassy_nrf::pac::FICR;
    let high = u64::from(ficr.deviceid(1).read());
    let addr = high << 32 | u64::from(ficr.deviceid(0).read());
    let addr = addr | 0x0000_c000_0000_0000;
    unwrap!(addr.to_le_bytes()[..6].try_into())
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Starting Charybdis Mini Right Half (Central)...");

    // Initialize nRF52840 peripherals
    let mut nrf_config = embassy_nrf::config::Config::default();
    nrf_config.dcdc.reg0_voltage = Some(embassy_nrf::config::Reg0Voltage::_3V3);
    nrf_config.dcdc.reg0 = true;
    nrf_config.dcdc.reg1 = true;
    let p = embassy_nrf::init(nrf_config);

    // Initialize MPSL and SoftDevice Controller
    let mpsl_p = mpsl::Peripherals::new(p.RTC0, p.TIMER0, p.TEMP, p.PPI_CH19, p.PPI_CH30, p.PPI_CH31);
    let lfclk_cfg = mpsl::raw::mpsl_clock_lfclk_cfg_t {
        source: mpsl::raw::MPSL_CLOCK_LF_SRC_RC as u8,
        rc_ctiv: mpsl::raw::MPSL_RECOMMENDED_RC_CTIV as u8,
        rc_temp_ctiv: mpsl::raw::MPSL_RECOMMENDED_RC_TEMP_CTIV as u8,
        accuracy_ppm: 500,
        skip_wait_lfclk_started: mpsl::raw::MPSL_DEFAULT_SKIP_WAIT_LFCLK_STARTED != 0,
    };
    static MPSL: StaticCell<MultiprotocolServiceLayer> = StaticCell::new();
    static SESSION_MEM: StaticCell<mpsl::SessionMem<1>> = StaticCell::new();
    let mpsl = MPSL.init(unwrap!(mpsl::MultiprotocolServiceLayer::with_timeslots(
        mpsl_p,
        Irqs,
        lfclk_cfg,
        SESSION_MEM.init(mpsl::SessionMem::new())
    )));
    spawner.spawn(mpsl_task(&*mpsl).unwrap());

    let sdc_p = sdc::Peripherals::new(
        p.PPI_CH17, p.PPI_CH18, p.PPI_CH20, p.PPI_CH21, p.PPI_CH22, p.PPI_CH23, p.PPI_CH24, p.PPI_CH25, p.PPI_CH26,
        p.PPI_CH27, p.PPI_CH28, p.PPI_CH29,
    );
    let mut rng = rng::Rng::new(p.RNG, Irqs);
    let mut sdc_mem = sdc::Mem::<15472>::new();
    let sdc = unwrap!(build_sdc(sdc_p, &mut rng, mpsl, &mut sdc_mem));

    // USB Driver (for optional wired connection / charging)
    let driver = Driver::new(p.USBD, Irqs, HardwareVbusDetect::new(Irqs));

    // Flash storage (Adafruit bootloader starts at 0x1000; storage placed at 0xA0000, 6 sectors = 24KB)
    let flash = Flash::take(mpsl, p.NVMC);

    // Matrix pins for Right Half:
    // Rows: Pro Micro 18, 5, 4, 9 -> P1.15, P0.24, P0.22, P1.06 (Outputs)
    // Cols: Pro Micro 19, 20, 10, 6, 7, 8 -> P0.02, P0.29, P0.09, P1.00, P0.11, P1.04 (Inputs pull-down)
    let (row_pins, col_pins) = config_matrix_pins_nrf!(
        peripherals: p,
        input: [P0_02, P0_29, P0_09, P1_00, P0_11, P1_04],
        output: [P1_15, P0_24, P0_22, P1_06]
    );

    // Battery ADC: nice!nano v2 AIN2 (P0.04) with 2M/806k voltage divider
    let adc_pin = p.P0_04.degrade_saadc();
    let saadc = init_adc(adc_pin, p.SAADC);
    saadc.calibrate().await;

    // Device and configuration
    let device_config = DeviceConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "BastardKB",
        product_name: "Charybdis Mini",
        ..DeviceConfig::default()
    };
    let ble_battery_config = BleBatteryConfig::new(None, true, None, false);
    let storage_config = StorageConfig {
        start_addr: 0xA0000,
        num_sectors: 6,
        clear_storage: false,
        ..Default::default()
    };
    let rmk_config = RmkConfig {
        device_config,
        ble_battery_config,
        storage_config,
        ..Default::default()
    };

    // Keymap and behavior initialization
    let mut keymap_data = KeymapData::new(keymap::get_default_keymap());
    let mut behavior_config = BehaviorConfig::default();
    behavior_config.morse.enable_flow_tap = true;
    behavior_config.morse.prior_idle_time = embassy_time::Duration::from_millis(125);
    behavior_config.morse.default_profile.set_hold_timeout_ms(120);
    behavior_config.morse.default_profile.set_gap_timeout_ms(180);

    // Auto-mouse layer: switches to Layer 1 (Mouse) on trackball motion
    let _ = behavior_config
        .auto_mouse_layer
        .push(AutoMouseLayerConfig::new(None, 1, embassy_time::Duration::from_millis(1000), 1));

    let key_config = PositionalConfig::default();
    let (keymap, mut storage) = initialize_keymap_and_storage(
        &mut keymap_data,
        flash,
        &storage_config,
        &mut behavior_config,
        &key_config,
    )
    .await;

    // Matrix scanner for Right Half:
    // 4 rows, 6 cols, diode direction row2col (COL2ROW = false)
    // ROW_OFFSET = 0, COL_OFFSET = 6 (keys emit cols 6..11)
    let debouncer = DefaultDebouncer::new();
    let mut matrix = Matrix::<_, _, _, 4, 6, false, 0, 6>::new(row_pins, col_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap);
    let host_service = HostService::new(&keymap, &rmk_config);

    // PMW3610 Trackball (Right Half):
    // SCK: P0.08, SDIO: P0.17, CS: P0.20, MOTION: P0.06
    let pmw3610_config = Pmw3610Config {
        res_cpi: 800,
        smart_mode: true,
        ..Default::default()
    };
    let pmw3610_sck = Output::new(p.P0_08, Level::High, OutputDrive::Standard);
    let pmw3610_sdio = Flex::new(p.P0_17);
    let pmw3610_cs = Output::new(p.P0_20, Level::High, OutputDrive::Standard);
    let pmw3610_motion = Some(Input::new(p.P0_06, Pull::Up));
    let pmw3610_spi = BitBangSpiBus::new(pmw3610_sck, pmw3610_sdio);
    let mut pmw3610_device =
        PointingDevice::<Pmw3610<_, _, _>>::new(0, pmw3610_spi, pmw3610_cs, pmw3610_motion, pmw3610_config);

    // Pointing processor with axes transform: swap_xy = true, invert_x = true, invert_y = true
    let pointing_processor_config = PointingProcessorConfig {
        device_id: 0,
        invert_x: true,
        invert_y: true,
        swap_xy: true,
    };
    let mut pointing_processor = PointingProcessor::new(&keymap, pointing_processor_config);
    let mut pointing_controller = PointingProcessorController::new();
    let mut auto_mouse_runner = AutoMouseLayerRunner::new(&keymap);

    // Battery processor for voltage monitoring (nice!nano v2 divider)
    let mut adc_device = NrfAdc::new(
        saadc,
        [AnalogEventType::Battery],
        [0],
        embassy_time::Duration::from_secs(12),
        None,
    );
    let mut batt_proc = BatteryProcessor::new(2000, 2806);

    let mut usb_transport = UsbTransport::new(driver, rmk_config.device_config).with_host_service(&host_service);

    // Split BLE Transport: connects to 1 peripheral half (Left half: 4x6 at row_offset 0, col_offset 0)
    // And advertises as a peripheral toward the active Dongle (or host BLE)
    let mut ble_transport = BleTransport::new(
        sdc,
        ble_addr(),
        rmk_config,
        [PeripheralMatrixConfig {
            rows: 4,
            cols: 6,
            row_offset: 0,
            col_offset: 0,
        }],
    )
    .with_host_service(&host_service);

    let mut wpm_processor = WpmProcessor::new();
    let mut watchdog_runner = Nrf52Watchdog::default_runner(p.WDT);

    info!("Charybdis Right Half initialized. Running...");

    run_all!(
        matrix,
        pmw3610_device,
        pointing_processor,
        pointing_controller,
        auto_mouse_runner,
        adc_device,
        batt_proc,
        storage,
        usb_transport,
        ble_transport,
        wpm_processor,
        keyboard,
        watchdog_runner
    )
    .await;
}
