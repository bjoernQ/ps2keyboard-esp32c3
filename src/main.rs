#![no_std]
#![no_main]

use core::cell::RefCell;

use const_queue::ConstQueue;
use esp32c3_hal::{
    gpio::{Gpio7, Gpio8},
    pac::Peripherals,
    prelude::*,
    RtcCntl, Timer, IO,
};
use esp_hal_common::{interrupt, pac, Cpu, Event, Floating, Input, Pin};
use esp_println::println;
use panic_halt as _;
use pc_keyboard::{layouts, HandleControl, ScancodeSet2};
use riscv::interrupt::Mutex;
use riscv_rt::entry;

static mut CLK: Mutex<RefCell<Option<Gpio7<Input<Floating>>>>> = Mutex::new(RefCell::new(None));
static mut DATA: Mutex<RefCell<Option<Gpio8<Input<Floating>>>>> = Mutex::new(RefCell::new(None));

#[entry]
fn main() -> ! {
    let peripherals = Peripherals::take().unwrap();

    // Disable the watchdog timers. For the ESP32-C3, this includes the Super WDT,
    // the RTC WDT, and the TIMG WDTs.
    let mut rtc_cntl = RtcCntl::new(peripherals.RTC_CNTL);
    let mut timer0 = Timer::new(peripherals.TIMG0);
    let mut timer1 = Timer::new(peripherals.TIMG1);

    rtc_cntl.set_super_wdt_enable(false);
    rtc_cntl.set_wdt_enable(false);
    timer0.disable();
    timer1.disable();

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
    let mut data_out = io.pins.gpio1.into_open_drain_output();
    let mut clk_out = io.pins.gpio2.into_open_drain_output();

    let data_in = io.pins.gpio8.into_floating_input();
    let mut clk_in = io.pins.gpio7.into_floating_input();
    clk_in.listen(Event::FallingEdge);

    data_out.set_low().unwrap();
    clk_out.set_low().unwrap();

    data_out.set_high().unwrap();
    clk_out.set_high().unwrap();

    riscv::interrupt::free(|_cs| unsafe {
        CLK.get_mut().replace(Some(clk_in));
        DATA.get_mut().replace(Some(data_in));
    });

    interrupt::enable(
        Cpu::ProCpu,
        pac::Interrupt::GPIO,
        interrupt::CpuInterrupt::Interrupt3,
    );
    interrupt::set_kind(
        Cpu::ProCpu,
        interrupt::CpuInterrupt::Interrupt3,
        interrupt::InterruptKind::Level,
    );
    interrupt::set_priority(
        Cpu::ProCpu,
        interrupt::CpuInterrupt::Interrupt3,
        interrupt::Priority::Priority1,
    );

    unsafe {
        riscv::interrupt::enable();
    }

    let mut kb = pc_keyboard::Keyboard::new(
        layouts::Us104Key,
        ScancodeSet2,
        HandleControl::MapLettersToUnicode,
    );
    loop {
        if let Some(byte) = get_byte() {
            match kb.add_byte(byte) {
                Ok(Some(event)) => {
                    println!("Event {:?}", event);
                }
                Ok(None) => (),
                Err(e) => {
                    println!("Error decoding: {:?}", e);
                }
            }
        }
    }
}

static mut QUEUE: Option<ConstQueue<u8, 5>> = None;

fn send_byte(byte: u8) {
    riscv::interrupt::free(|_| unsafe {
        if QUEUE.is_none() {
            QUEUE = Some(ConstQueue::new());
        }
        match QUEUE {
            Some(ref mut queue) => {
                queue.push(byte).ok();
            }
            None => (),
        }
    });
}

fn get_byte() -> Option<u8> {
    riscv::interrupt::free(|_| unsafe {
        match QUEUE {
            Some(ref mut queue) => queue.pop().ok(),
            None => None,
        }
    })
}

#[no_mangle]
pub fn interrupt3() {
    static mut BIT_COUNT: usize = 0;
    static mut CURRENT: u8 = 0;

    riscv::interrupt::free(|cs| unsafe {
        let mut clk = CLK.borrow(*cs).borrow_mut();
        let clk = clk.as_mut().unwrap();

        let mut data = DATA.borrow(*cs).borrow_mut();
        let data = data.as_mut().unwrap();

        let bit = if data.is_high().unwrap() { 1 } else { 0 };

        interrupt::clear(Cpu::ProCpu, interrupt::CpuInterrupt::Interrupt3);
        clk.clear_interrupt();

        if BIT_COUNT > 0 && BIT_COUNT < 9 {
            CURRENT = CURRENT.overflowing_shr(1).0;
            CURRENT |= bit << 7;
        }
        BIT_COUNT += 1;

        if BIT_COUNT == 11 {
            send_byte(CURRENT);

            BIT_COUNT = 0;
            CURRENT = 0;
        }
    });
}
