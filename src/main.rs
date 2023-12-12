#![no_std]
#![no_main]

use core::cell::RefCell;

use core::mem::MaybeUninit;
use critical_section::Mutex;
use hal::{
    clock::{ClockControl, CpuClock},
    gpio::{Event, Gpio1, Gpio2, OpenDrain, Output},
    interrupt,
    peripherals::{self, Peripherals},
    prelude::*,
    Cpu, IO,
    uart::{
        config::{Config, DataBits, Parity, StopBits},
        TxRxPins,
    },
    Uart
};
use esp_backtrace as _;
use pc_keyboard::{layouts, HandleControl, ScancodeSet2, KeyEvent, KeyCode};
use log::{info, error};

static CLK: Mutex<RefCell<Option<Gpio2<Output<OpenDrain>>>>> = Mutex::new(RefCell::new(None));
static DATA: Mutex<RefCell<Option<Gpio1<Output<OpenDrain>>>>> = Mutex::new(RefCell::new(None));
static QUEUE: Mutex<RefCell<Option<SimpleQueue<u8, 5>>>> = Mutex::new(RefCell::new(None));

// Maps keycodes to ASCII characters
fn map_keycode(keycode: KeyCode) -> Option<char> {
    match keycode {
        KeyCode::ArrowUp => Some('7'),
        KeyCode::ArrowDown => Some('6'),
        KeyCode::ArrowRight => Some('8'),
        KeyCode::ArrowLeft => Some('5'),
        KeyCode::Backspace => Some('\x08'),
        KeyCode::Tab => Some('\t'),
        KeyCode::Return => Some('\n'),
        KeyCode::Escape => Some('\x1B'),
        KeyCode::Home => Some('H'),
        KeyCode::End => Some('F'),
        KeyCode::PageUp => Some('I'),
        KeyCode::PageDown => Some('G'),
        KeyCode::Insert => Some('L'),
        KeyCode::Delete => Some('M'),
        KeyCode::F1 => Some('P'),
        KeyCode::F2 => Some('Q'),
        KeyCode::F3 => Some('R'),
        KeyCode::F4 => Some('S'),
        KeyCode::F5 => Some('T'),
        KeyCode::F6 => Some('U'),
        KeyCode::F7 => Some('V'),
        KeyCode::F8 => Some('W'),
        KeyCode::F9 => Some('X'),
        KeyCode::F10 => Some('Y'),
        KeyCode::F11 => Some('Z'),
        KeyCode::F12 => Some('['),
        KeyCode::PrintScreen => Some(']'),
        KeyCode::PauseBreak => Some('\\'),
        KeyCode::CapsLock => Some('`'),
        // KeyCode::NumLock => Some('~'),
        KeyCode::ScrollLock => Some('!'),
        // KeyCode::BackTick => Some('@'),
        // KeyCode::Minus => Some('#'),
        // KeyCode::Equal => Some('$'),
        // KeyCode::LeftSquareBracket => Some('%'),
        // KeyCode::RightSquareBracket => Some('^'),
        // KeyCode::BackSlash => Some('&'),
        // KeyCode::Semicolon => Some(';'),
        // KeyCode::Quote => Some('*'),
        // KeyCode::Comma => Some('('),
        // KeyCode::Slash => Some('_'),
        KeyCode::Spacebar => Some(' '),
        KeyCode::Key1 => Some('1'),
        KeyCode::Key2 => Some('2'),
        KeyCode::Key3 => Some('3'),
        KeyCode::Key4 => Some('4'),
        KeyCode::Key5 => Some('5'),
        KeyCode::Key6 => Some('6'),
        KeyCode::Key7 => Some('7'),
        KeyCode::Key8 => Some('8'),
        KeyCode::Key9 => Some('9'),
        KeyCode::Key0 => Some('0'),
        KeyCode::A => Some('a'),
        KeyCode::B => Some('b'),
        KeyCode::C => Some('c'),
        KeyCode::D => Some('d'),
        KeyCode::E => Some('e'),
        KeyCode::F => Some('f'),
        KeyCode::SysRq => None,
        KeyCode::Oem8 => None,
        KeyCode::OemMinus => None,
        KeyCode::OemPlus => None,
        KeyCode::NumpadLock => None,
        KeyCode::NumpadDivide => None,
        KeyCode::NumpadMultiply => None,
        KeyCode::NumpadSubtract => None,
        KeyCode::Q => Some('q'),
        KeyCode::W => Some('w'),
        KeyCode::R => Some('r'),
        KeyCode::T => Some('s'),
        KeyCode::Y => Some('t'),
        KeyCode::U => Some('y'),
        KeyCode::I => Some('i'),
        KeyCode::O => Some('o'),
        KeyCode::P => Some('p'),
        KeyCode::Oem4 => None,
        KeyCode::Oem6 => None,
        KeyCode::Oem5 => None,
        KeyCode::Oem7 => None,
        KeyCode::Numpad7 => Some('7'),
        KeyCode::Numpad8 => Some('8'),
        KeyCode::Numpad9 => Some('9'),
        KeyCode::NumpadAdd => Some('+'),
        KeyCode::S => Some('s'),
        KeyCode::G => Some('g'),
        KeyCode::H => Some('h'),
        KeyCode::J => Some('j'),
        KeyCode::K => Some('k'),
        KeyCode::L => Some('l'),
        KeyCode::Oem1 => None,
        KeyCode::Oem3 => None,
        KeyCode::Numpad4 => Some('4'),
        KeyCode::Numpad5 => Some('5'),
        KeyCode::Numpad6 => Some('6'),
        KeyCode::LShift => None,
        KeyCode::Z => Some('z'),
        KeyCode::X => Some('x'),
        KeyCode::V => Some('v'),
        KeyCode::N => Some('n'),
        KeyCode::M => Some('m'),
        KeyCode::OemComma => Some(','),
        KeyCode::OemPeriod => Some('.'),
        KeyCode::Oem2 => None,
        KeyCode::RShift => None,
        KeyCode::Numpad1 => Some('1'),
        KeyCode::Numpad2 => Some('2'),
        KeyCode::Numpad3 => Some('3'),
        KeyCode::NumpadEnter => Some('\n'),
        KeyCode::LControl => None,
        KeyCode::LWin => None,
        KeyCode::LAlt => None,
        KeyCode::RAltGr => None,
        KeyCode::RWin => None,
        KeyCode::Apps => None,
        KeyCode::RControl => None,
        KeyCode::Numpad0 => None,
        KeyCode::NumpadPeriod => None,
        KeyCode::Oem9 => None,
        KeyCode::Oem10 => None,
        KeyCode::Oem11 => None,
        KeyCode::Oem12 => None,
        KeyCode::Oem13 => None,
        KeyCode::PrevTrack => None,
        KeyCode::NextTrack => None,
        KeyCode::Mute => None,
        KeyCode::Calculator => None,
        KeyCode::Play => None,
        KeyCode::Stop => None,
        KeyCode::VolumeDown => None,
        KeyCode::VolumeUp => None,
        KeyCode::WWWHome => None,
        KeyCode::PowerOnTestOk => None,
        KeyCode::TooManyKeys => None,
        KeyCode::RControl2 => None,
        KeyCode::RAlt2 => None,
    }
}


#[entry]
fn main() -> ! {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();
    // let clocks = ClockControl::configure(system.clock_control, CpuClock::Clock80MHz).freeze();

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
    let mut data = io.pins.gpio1.into_open_drain_output();
    let mut clk = io.pins.gpio2.into_open_drain_output();

    esp_println::logger::init_logger_from_env();

    info!("Starting");

    let serial_tx = io.pins.gpio5.into_push_pull_output();
    let serial_rx = io.pins.gpio4.into_floating_input();

    let pins = TxRxPins::new_tx_rx(
        serial_tx,
        serial_rx,
    );

    let config = Config {
        baudrate: 115200,
        data_bits: DataBits::DataBits8,
        parity: Parity::ParityNone,
        stop_bits: StopBits::STOP1,
    };

    let mut serial = Uart::new_with_config(peripherals.UART1, config, Some(pins), &clocks);
    // serial.write("Hello, world!\n").unwrap();
    serial.write(0x0A).unwrap();

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

    // let encode_config = bincode::config::standard();
    loop {
        if let Some(byte) = get_byte() {
            match kb.add_byte(byte) {
                Ok(Some(event)) => {
                    // let data = bincode::encode_into_slice(&event, encode_config).unwrap();
                    // serial.write(&data).unwrap();
                    // info!("Event {:?}, Byte: {}", event, byte);


                    match event.state {
                        pc_keyboard::KeyState::Up => {
                            match map_keycode(event.code) {
                                Some(keycode_event) => {
                                    info!("Sending: Up {}", keycode_event);
                                    serial.write(b'1').unwrap();
                                    serial.write(keycode_event as u8).unwrap();
                                },
                                None => {}
                            }
                        },
                        pc_keyboard::KeyState::Down => {
                            match map_keycode(event.code) {
                                Some(keycode_event) => {
                                    info!("Sending: Down {}", keycode_event);
                                    serial.write(b'0').unwrap();
                                    serial.write(keycode_event as u8).unwrap();
                                },
                                None => {}
                            }
                        }
,
                        pc_keyboard::KeyState::SingleShot => {},
                    }

                }
                Ok(None) => (),
                Err(e) => {
                    error!("Error decoding: {:?}", e);
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
