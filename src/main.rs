#![no_std]
#![no_main]

use core::marker::PhantomData;

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
        defmt::todo!("Implement bind method")
    }
}

impl Vault<Locked> {
    // Secret should be introduced when successfully unlocked.
    pub fn unlock(self, pin: [u8; 2]) -> Result<Vault<Unlocked>, Vault<Locked>> {
        defmt::todo!("Implement unlock method")
    }
}

fn generate_pin<T: CryptoRng>(rng: T) -> [u8; 2] {
    defmt::todo!("Implement PIN generation logic")
}

#[entry]
fn main() -> ! {
    info!("eCTF MP1 started");

    let p = embassy_mspm0::init(Default::default());

    let mut trng = Trng::new(p.TRNG).expect("Failed to initialize TRNG");

    loop {
        let vault: Vault<Unbound> = Default::default();
        // ...wait for x command
        let pin = generate_pin(trng.unwrap_mut());
        let mut vault = vault.bind(pin);
        // TODO: blink LED to show pin...
        let vault = loop {
            // ...wait for g__ command
            let pin = [0u8; 2]; // TODO: get pin from command
            match vault.unlock(pin) {
                Ok(unlocked_vault) => break unlocked_vault,
                Err(locked_vault) => vault = locked_vault,
            }
        };
        loop {
            // ... wait for command
            // If q, write vault.secret to UART
            // If u, break to reset vault
        }
    }
}
