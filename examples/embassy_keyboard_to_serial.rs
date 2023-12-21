#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use embassy_executor::Spawner;
use hal::{
    clock::ClockControl, gpio::{Gpio1, Gpio2, OpenDrain, Output},
    peripherals::{Peripherals, UART1}, prelude::*, uart::{config::{Config, DataBits, Parity, StopBits}, UartTx}, Uart, IO,
};
use esp_backtrace as _;
use embassy_time::{Duration, Timer};
use hal::uart::TxRxPins;
use embassy_sync::pipe::Pipe;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use log::{error, info};
use embedded_hal_async::digital::Wait;

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

    info!("PS2 Reader started");
    loop {
        // Asynchronously wait for falling edge on the clock line
        clk.wait_for_falling_edge().await.unwrap();
        info!("Falling edge");

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
            info!("Sending byte: {}", current_byte);
            let bytes_written = PIPE.write(&[current_byte]).await;
            if bytes_written != 1 {
                panic!("Failed to write to Pipe");
            }
            bit_count = 0;
            current_byte = 0;
        }
    }
}



#[main]
async fn main(spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    esp_println::logger::init_logger_from_env();

    info!("Starting");

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
    let mut clk = io.pins.gpio2.into_open_drain_output();
    let mut data = io.pins.gpio1.into_open_drain_output();
    let serial_tx = io.pins.gpio5.into_push_pull_output();
    let serial_rx = io.pins.gpio4.into_floating_input();
    let uart_pins = TxRxPins::new_tx_rx(serial_tx, serial_rx);
    let mut uart = Uart::new_with_config(peripherals.UART1, Config { baudrate: 115200, data_bits: DataBits::DataBits8, parity: Parity::ParityNone, stop_bits: StopBits::STOP1, }, Some(uart_pins), &clocks);
    let (uart_tx, _uart_rx) = uart.split();

    // Spawn the tasks
    spawner.spawn(ps2_reader(data, clk)).unwrap();
    spawner.spawn(uart_writer(uart_tx)).unwrap();
}