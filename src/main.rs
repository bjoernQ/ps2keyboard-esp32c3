#![no_std]
#![no_main]

use core::cell::RefCell;

use core::mem::MaybeUninit;
use critical_section::Mutex;
use esp_backtrace as _;
use esp_println::println;
use hal::{
    gpio::{Event, Gpio1, Gpio2, OpenDrain, Output},
    interrupt,
    peripherals::{self, Peripherals},
    prelude::*,
    Cpu, IO,
};
use pc_keyboard::{layouts, HandleControl, ScancodeSet2};

static CLK: Mutex<RefCell<Option<Gpio2<Output<OpenDrain>>>>> = Mutex::new(RefCell::new(None));
static DATA: Mutex<RefCell<Option<Gpio1<Output<OpenDrain>>>>> = Mutex::new(RefCell::new(None));
static QUEUE: Mutex<RefCell<Option<SimpleQueue<u8, 5>>>> = Mutex::new(RefCell::new(None));

#[entry]
fn main() -> ! {
    let peripherals = Peripherals::take();

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
    let mut data = io.pins.gpio1.into_open_drain_output();
    let mut clk = io.pins.gpio2.into_open_drain_output();

    clk.listen(Event::FallingEdge);

    data.set_low().unwrap();
    clk.set_low().unwrap();

    data.set_high().unwrap();
    clk.set_high().unwrap();

    critical_section::with(|cs| {
        CLK.borrow_ref_mut(cs).replace(clk);
        DATA.borrow_ref_mut(cs).replace(data);
    });

    interrupt::enable(peripherals::Interrupt::GPIO, interrupt::Priority::Priority3).unwrap();

    unsafe {
        hal::riscv::interrupt::enable();
    }

    let mut kb = pc_keyboard::Keyboard::new(
        ScancodeSet2::new(),
        layouts::Us104Key,
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

fn send_byte(byte: u8) {
    critical_section::with(|cs| {
        let mut queue = QUEUE.borrow_ref_mut(cs);

        if queue.is_none() {
            queue.replace(SimpleQueue::new());
        }

        queue.as_mut().unwrap().enqueue(byte);
    });
}

fn get_byte() -> Option<u8> {
    critical_section::with(|cs| match *QUEUE.borrow_ref_mut(cs) {
        Some(ref mut queue) => queue.dequeue(),
        None => None,
    })
}

#[interrupt]
fn GPIO() {
    static mut BIT_COUNT: usize = 0;
    static mut CURRENT: u8 = 0;

    critical_section::with(|cs| unsafe {
        let mut clk = CLK.borrow_ref_mut(cs);
        let clk = clk.as_mut().unwrap();

        let mut data = DATA.borrow_ref_mut(cs);
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
