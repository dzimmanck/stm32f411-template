// #![no_main]
// #![no_std]

use ecdc::process_cmd;

// ritc imports
use crate::app::{ecdc_task, usart};
use rtic::mutex_prelude::*;

use stm32f4xx_hal::{nb::block, prelude::*, serial::Error};

// UART interrupt, read from the RX buffer and write to the queue
// #[task(binds = USART1, local = [rx])]
pub(crate) fn usart(cx: usart::Context) {
    match block!(cx.local.rx.read()) {
        Ok(byte) => {
            ecdc_task::spawn(byte).ok();
        }
        Err(error) => match error {
            Error::Framing => defmt::info!("Framing error in UART"),
            Error::Noise => defmt::info!("Noise error in UART"),
            Error::Overrun => defmt::info!("Overrun error in UART"),
            Error::Parity => defmt::info!("Parity error in UART"),
            _ => defmt::info!("Unknown error in UART"),
        },
    }
}

// #[task(capacity = 1, local = [tx, ecdc_receiver], shared=[models])]
pub(crate) fn ecdc_task(mut cx: ecdc_task::Context, byte: u8) {
    let response = cx.local.ecdc_receiver.add_byte(byte);

    // receiver will return message once entire packet is received
    if let Some(message) = response {
        // defmt::info!("Received -> {:?}", message);

        let reply = cx
            .shared
            .models
            .lock(|models| process_cmd(&message, models));

        reply
            .iter()
            .for_each(|&byte| match block!(cx.local.tx.write(byte)) {
                Ok(_) => (),
                Err(_) => {
                    // defmt::info!("Problem sending {:?}", byte);
                }
            });
    }
}
