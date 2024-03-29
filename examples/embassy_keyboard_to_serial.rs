#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_time::Timer;
use embedded_hal_async::digital::Wait;
use esp_backtrace as _;
use hal::embassy;
use hal::uart::TxRxPins;
use hal::{
    clock::{ClockControl, CpuClock},
    gpio::{Gpio1, Gpio2, OpenDrain, Output},
    peripherals::{Peripherals, UART1},
    prelude::*,
    uart::{
        config::{Config, DataBits, Parity, StopBits},
        UartTx,
    },
    Uart, IO,
};
use log::{error, info};

// Size of the Pipe buffer
const PIPE_BUF_SIZE: usize = 5;

// Initialize the Pipe globally
static PIPE: Pipe<CriticalSectionRawMutex, PIPE_BUF_SIZE> = Pipe::new();

#[embassy_executor::task]
async fn uart_writer(mut uart: UartTx<'static, UART1>) {
    info!("UART Writer started");
    loop {
        let mut byte = [0u8];
        let bytes_read = PIPE.read(&mut byte).await;
        if bytes_read > 0 {
            match uart.write(byte[0]) {
                Ok(_) => info!("Sent byte: {}", byte[0]),
                Err(_) => error!("Error sending byte: {}", byte[0]),
            }
        }
    }
}

#[embassy_executor::task]
async fn ps2_reader(mut data: Gpio1<Output<OpenDrain>>, mut clk: Gpio2<Output<OpenDrain>>) {
    let mut bit_count: usize = 0;
    let mut current_byte: u8 = 0;

    info!("PS/2 Reader started");

    data.set_low().unwrap();
    clk.set_low().unwrap();

    data.set_high().unwrap();
    clk.set_high().unwrap();

    Timer::after_millis(250).await;

    info!("Waiting for PS/2 signal");

    loop {
        // Asynchronously wait for falling edge on the clock line
        match clk.wait_for_falling_edge().await {
            Ok(_) => {
                // Reading data on falling edge
                let bit = if data.is_high().unwrap() { 1 } else { 0 };

                // Assemble the byte
                if bit_count > 0 && bit_count < 9 {
                    current_byte >>= 1;
                    current_byte |= bit << 7;
                }

                bit_count += 1;

                // Once a full byte is received
                if bit_count == 11 {
                    let bytes_written = PIPE.write(&[current_byte]).await;
                    if bytes_written != 1 {
                        error!("Failed to write to pipe");
                        break;
                    }
                    bit_count = 0;
                    current_byte = 0;
                }
            }
            Err(_) => error!("Error waiting for falling edge"),
        }
    }
}

#[main]
async fn main(spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();

    // The clock must operate at 160MHz to make PS/2 decoding work in combination with Embassy
    let clocks = ClockControl::configure(system.clock_control, CpuClock::Clock160MHz).freeze();

    esp_println::logger::init_logger_from_env();

    info!("Starting");

    embassy::init(
        &clocks,
        hal::timer::TimerGroup::new(peripherals.TIMG0, &clocks).timer0,
    );

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
    let clk = io.pins.gpio2.into_open_drain_output();
    let data = io.pins.gpio1.into_open_drain_output();
    let serial_tx = io.pins.gpio5.into_push_pull_output();
    let serial_rx = io.pins.gpio4.into_floating_input();
    let uart_pins = TxRxPins::new_tx_rx(serial_tx, serial_rx);
    let uart_config = Config {
        baudrate: 115200,
        data_bits: DataBits::DataBits8,
        parity: Parity::ParityNone,
        stop_bits: StopBits::STOP1,
    };
    let uart = Uart::new_with_config(peripherals.UART1, uart_config, Some(uart_pins), &clocks);
    let (uart_tx, _uart_rx) = uart.split();

    // Spawn the tasks
    spawner.spawn(ps2_reader(data, clk)).unwrap();
    spawner.spawn(uart_writer(uart_tx)).unwrap();
}
