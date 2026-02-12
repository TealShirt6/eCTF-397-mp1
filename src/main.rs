#![no_std]
#![no_main]

use core::marker::PhantomData;


use embassy_mspm0::gpio::{Level, Output};
use cortex_m::asm::delay;
use embassy_mspm0::uart::{Config, Uart};


use cortex_m_rt::entry;
use defmt::*;
use embassy_mspm0::trng::Trng;
use rand_core::{TryRngCore, CryptoRng};
use {defmt_rtt as _, panic_halt as _};

trait VaultState {}
struct Unbound;
struct Locked;
struct Unlocked;

impl VaultState for Unbound {}
impl VaultState for Locked {}
impl VaultState for Unlocked {}

struct Vault<State: VaultState> {
    pin: [u8; 2],
    failed_attempts: u32,
    secret: &'static str,
    _state: PhantomData<State>,
}

impl Default for Vault<Unbound> {
    fn default() -> Self {
        Vault {
            pin: [0; 2],
            secret: "",
            failed_attempts: 0,
            _state: core::marker::PhantomData,
        }
    }
}

impl Vault<Unbound> {
    pub fn bind(self, pin: [u8; 2]) -> Vault<Locked> {
        Vault {
            pin: pin,
            secret: "",
            failed_attempts: 0,
            _state: core::marker::PhantomData,
        }
    }
}

impl Vault<Locked> {
    // Secret should be introduced when successfully unlocked.
    pub fn unlock(self, pin: [u8; 2]) -> Result<Vault<Unlocked>, Vault<Locked>> {
        // Return unlocked vault as Ok
        if pin == self.pin {
            Ok(Vault {
            pin: self.pin,
            secret: "Aww man you found my secret!",
            failed_attempts: self.failed_attempts,
            _state: core::marker::PhantomData,
        })

        // Return locked vault with incremented failed attempts as Err
        } else {
            Err(Vault {
            pin: self.pin,
            secret: "",
            failed_attempts: self.failed_attempts + 1,
            _state: core::marker::PhantomData,
        })
        }
    }
}

fn generate_pin<T: CryptoRng>(mut rng: T) -> [u8; 2] {
    let mut pin = [0; 2];
    pin[0] = (rng.next_u32() % 4 + 1) as u8;
    pin[1] = (rng.next_u32() % 4 + 1) as u8;

    return pin;
}


// Implements vault using typestate pattern so methods unlock, bind, and default are only defined in certain states
// Currently reads doesn't account for user input error (ie: byes shorter than x\r\n)
#[entry]
fn main() -> ! {
    let clock_speed = 5000000;

    info!("eCTF MP1 started");

    let p = embassy_mspm0::init(Default::default());

    let instance = p.UART0;
    let tx = p.PA10;
    let rx = p.PA11;

    let config = Config::default();
    let mut uart: Uart<'_, embassy_mspm0::mode::Blocking> = unwrap!(Uart::new_blocking(instance, rx, tx, config));

    fn print_invalid_command(uart: &mut Uart<'_, embassy_mspm0::mode::Blocking>) {
        let error_string = "Invalid Command".as_bytes();
        unwrap!(uart.blocking_write(&error_string));
    };

    let mut trng = Trng::new(p.TRNG).expect("Failed to initialize TRNG");


    let mut led1 = Output::new(p.PA0, Level::Low);
    led1.set_inversion(true);
    led1.set_low();

    loop {
        let vault: Vault<Unbound> = Default::default();
        
        fn read_x(uart: &mut Uart<'_, embassy_mspm0::mode::Blocking>) -> bool {
            let mut buf: [u8; _] = [0u8; 1];

            match (uart.blocking_read(&mut buf)) {
                Err(e) => return false,
                Ok(e) => {}
            }
            if buf[0] != b'x' { return false; }

            unwrap!(uart.blocking_read(&mut buf));
            if buf[0] != b'\r' { return false; }

            unwrap!(uart.blocking_read(&mut buf));
            if buf[0] != b'\n' { return false; }

            return true
        };

        // Check whether we recieved bytes x\r\n
        if !read_x(&mut uart) {
            let error_string = "Invalid Command".as_bytes();
            unwrap!(uart.blocking_write(&error_string));
            continue;
        }

        let pin: [u8; 2] = generate_pin(trng.unwrap_mut());
        let mut vault = vault.bind(pin);

        // Notify that the device is bound
        let bound_string = "Device Bound".as_bytes();
        unwrap!(uart.blocking_write(&bound_string));

        // Blink LED to show pin
        for i in 0..2 {
            for _ in 0..pin[i] {
                led1.set_high();
                delay(clock_speed * 1);
                led1.set_low();
                delay(clock_speed * 1);
            }
            delay(clock_speed * 1)
        }

        let vault = loop {
            fn get_pin_attempt(uart: &mut Uart<'_, embassy_mspm0::mode::Blocking>) -> Result<[u8; 2], ()> {
                let mut buf: [u8; _] = [0u8; 1];
                let mut pin_buf: [u8; 2] = [0u8; 2];

                match (uart.blocking_read(&mut buf)) {
                Err(e) => return Err(()),
                Ok(e) => {}
            }
                if buf[0] != b'g' { return Err(()); }

                unwrap!(uart.blocking_read(&mut buf));
                if buf[0] <= 48 || buf[0] > 52 { return Err(()); }

                pin_buf[0] = buf[0];

                unwrap!(uart.blocking_read(&mut buf));
                if buf[0] <= 48 || buf[0] > 52 { return Err(()); }

                pin_buf[1] = buf[0];
                
                unwrap!(uart.blocking_read(&mut buf));
                if buf[0] != b'\r' { return Err(()); }

                unwrap!(uart.blocking_read(&mut buf));
                if buf[0] != b'\n' { return Err(()); }

                pin_buf = pin_buf.map(|v| v - 48);

                return Ok(pin_buf);
            };
            
            match get_pin_attempt(&mut uart) {
                Ok(pin) => {
                    match vault.unlock(pin) {
                        Ok(unlocked_vault) => {
                            let unlocked_string = ("Device unlocked").as_bytes();
                            unwrap!(uart.blocking_write(&unlocked_string));
                            break unlocked_vault
                        } ,
                        Err(locked_vault) => {
                            vault = locked_vault;
                            let err_string = ("Incorrect pin\r\n").as_bytes();
                            unwrap!(uart.blocking_write(&err_string));
                        }
                    }
                }
                Err(_) => {
                    print_invalid_command(&mut uart);
                    continue
                }
            }
        };
        loop {
            let mut get_char = || -> Result<u8, ()> {
                let mut buf: [u8; _] = [0u8; 1];
                let char: u8;

                match (uart.blocking_read(&mut buf)) {
                Err(e) => return Err(()),
                Ok(e) => {}
                }
                if buf[0] != b'q' && buf[0] != b'u' { return Err(()); }

                char = buf[0];

                unwrap!(uart.blocking_read(&mut buf));
                if buf[0] != b'\r' { return Err(()); }

                unwrap!(uart.blocking_read(&mut buf));
                if buf[0] != b'\n' { return Err(()); }

                return Ok(char);
            };

            match get_char() {
                Ok(b'q') => {
                    let secret = vault.secret.as_bytes();
                    unwrap!(uart.blocking_write(&secret));
                }
                Ok(b'u') => break,
                Ok(_) | Err(()) => {
                    print_invalid_command(&mut uart);
                    continue
                }
            }
        }
    }
}
