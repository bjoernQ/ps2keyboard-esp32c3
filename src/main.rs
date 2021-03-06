#![no_std]
#![no_main]

use core::cell::RefCell;

use core::mem::MaybeUninit;
use esp32c3_hal::{
    gpio::{Gpio1, Gpio2},
    gpio_types::{Event, OpenDrain, Output, Pin},
    interrupt,
    pac::{self, Peripherals},
    prelude::*,
    Cpu, RtcCntl, Timer, IO,
};
use esp_println::println;
use panic_halt as _;
use pc_keyboard::{layouts, HandleControl, ScancodeSet2};
use riscv::interrupt::Mutex;
use riscv_rt::entry;

static mut CLK: Mutex<RefCell<Option<Gpio2<Output<OpenDrain>>>>> = Mutex::new(RefCell::new(None));
static mut DATA: Mutex<RefCell<Option<Gpio1<Output<OpenDrain>>>>> = Mutex::new(RefCell::new(None));

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
    let mut data = io.pins.gpio1.into_open_drain_output();
    let mut clk = io.pins.gpio2.into_open_drain_output();

    clk.listen(Event::FallingEdge);

    data.set_low().unwrap();
    clk.set_low().unwrap();

    data.set_high().unwrap();
    clk.set_high().unwrap();

    riscv::interrupt::free(|_cs| unsafe {
        CLK.get_mut().replace(Some(clk));
        DATA.get_mut().replace(Some(data));
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

static mut QUEUE: Option<SimpleQueue<u8, 5>> = None;

fn send_byte(byte: u8) {
    riscv::interrupt::free(|_| unsafe {
        if QUEUE.is_none() {
            QUEUE = Some(SimpleQueue::new());
        }
        match QUEUE {
            Some(ref mut queue) => {
                queue.enqueue(byte);
            }
            None => (),
        }
    });
}

fn get_byte() -> Option<u8> {
    riscv::interrupt::free(|_| unsafe {
        match QUEUE {
            Some(ref mut queue) => queue.dequeue(),
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

pub struct SimpleQueue<T, const N: usize> {
    data: [Option<T>; N],
    read_index: usize,
    write_index: usize,
}

impl<T, const N: usize> SimpleQueue<T, N> {
    pub fn new() -> SimpleQueue<T, N> {
        let mut queue = unsafe {
            SimpleQueue {
                data: MaybeUninit::uninit().assume_init(),
                read_index: 0,
                write_index: 0,
            }
        };

        for i in 0..N {
            queue.data[i] = None;
        }

        queue
    }

    pub fn enqueue(&mut self, e: T) -> bool {
        self.data[self.write_index] = Some(e);

        self.write_index += 1;
        self.write_index %= N;

        if self.write_index == self.read_index {
            return false;
        }

        true
    }

    pub fn dequeue(&mut self) -> Option<T> {
        if self.write_index == self.read_index {
            None
        } else {
            let result = self.data[self.read_index].take();
            self.read_index += 1;
            self.read_index %= N;
            result
        }
    }

    pub fn is_empty(&self) -> bool {
        self.read_index == self.write_index
    }

    pub fn is_full(&self) -> bool {
        let mut next_write = self.read_index + 1;
        next_write %= N;

        next_write == self.read_index
    }
}
