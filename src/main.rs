#![no_std]
#![no_main]

use core::marker::PhantomData;


use embassy_mspm0::gpio::{Level, Output};
use cortex_m::asm::delay;
use embassy_mspm0::uart::{Config, Uart, Error};


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
    let mut uart = unwrap!(Uart::new_blocking(instance, rx, tx, config));


    let mut trng = Trng::new(p.TRNG).expect("Failed to initialize TRNG");


    let mut led1 = Output::new(p.PA0, Level::Low);
    led1.set_inversion(true);
    led1.set_low();

    loop {
        let vault: Vault<Unbound> = Default::default();
        let mut buf: [u8; _] = [0u8; 3];

        // Wait for 'x', continue if different character recieved
        // buf len is 3 to read in all 3 bytes of x\r\n
        unwrap!(uart.blocking_read(&mut buf));
        unwrap!(uart.blocking_write(&buf));
        if buf[0] != b'x' { continue }


        let pin = generate_pin(trng.unwrap_mut());
        let mut vault = vault.bind(pin);

        

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
            let mut buf: [u8; _] = [0u8; 5];

            // Reads 5 bytes
            // Check for errors
            match uart.blocking_read(&mut buf) {
                Ok(e) => {info!("{}", e)}
                Err(Error::Overrun) => {
                    info!("Overrun, buf: {}", buf)
                }
                Err(_) => {}
            }

            unwrap!(uart.blocking_write(&buf));
            if (buf[0] != b'g') { continue }
            let mut pin: [u8; 2] = (&buf[1..3]).try_into().unwrap();
            pin = pin.map(|v| v - 48);
            info!("pin: {}", pin);
            match vault.unlock(pin) {
                Ok(unlocked_vault) => break unlocked_vault,
                Err(locked_vault) => vault = locked_vault,
            }
            
            let err_string = ("Incorrect pin\r\n").as_bytes();
            unwrap!(uart.blocking_write(&err_string));
        };
        loop {
            let mut buf: [u8; _] = [0u8; 3];

            // Wait for 'x', continue if different character recieved
            unwrap!(uart.blocking_read(&mut buf));
            unwrap!(uart.blocking_write(&buf));

            // If inputted 'q', write secret to UART
            if buf[0] == b'q' {
                let secret = vault.secret.as_bytes();
                unwrap!(uart.blocking_write(&secret));

            // If inputted 'u', break loop
            } else if buf[0] == b'u' {
                break
            }   
        }
    }
}
