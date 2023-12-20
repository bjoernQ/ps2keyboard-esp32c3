#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, signal::Signal};
use hal::{
    clock::ClockControl,
    embassy,
    gpio::{Event, Gpio1, Gpio2, OpenDrain, Output},
    peripherals::{self, Peripherals, UART0},
    prelude::*,
    uart::{
        config::{Config, DataBits, Parity, StopBits},
        UartRx, UartTx,
    },
    Uart,
    IO,
};
use esp_backtrace as _;
use static_cell::make_static;

#[embassy_executor::task]
async fn uart_writer(
    mut uart: UartTx<'static, UART0>,
    receiver: Receiver<u8>
) {
    loop {
        // Receive a byte from ps2_reader
        let byte = receiver.receive().await;
        // Send the byte over UART
        uart.write(&[byte]).await.unwrap();
    }
}

#[embassy_executor::task]
async fn ps2_reader(
    mut data: Gpio1<Output<OpenDrain>>,
    mut clk: Gpio2<Output<OpenDrain>>,
    sender: Sender<u8>
) {
    loop {

    }
}

#[embassy_executor::task]
async fn main(spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    // Correcting GPIO access
    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
    let mut clk = io.pins.gpio2.into_open_drain_output();
    let mut data = io.pins.gpio1.into_open_drain_output();

    // Initialize UART...
    let mut uart = Uart::new_with_config(
        peripherals.UART1,
        Config {
            baudrate: 115200,
            data_bits: DataBits::DataBits8,
            parity: Parity::ParityNone,
            stop_bits: StopBits::STOP1,
        },
        None, // TODO: Update for Embassy
        &clocks,
    );

}
