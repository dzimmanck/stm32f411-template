#![no_main]
#![no_std]

use {{crate_name}} as _; // memory layout + panic handler

// submodules for externally defined tasks
mod uart;

#[rtic::app(device = stm32f4xx_hal::stm32, peripherals = true, dispatchers = [EXTI0, EXTI1, EXTI2])]
mod app {
    // use defmt::Format;
    use dwt_systick_monotonic::*;
    use stm32f4xx_hal::{
        gpio::{gpioa::PA5, Output, PushPull},
        prelude::*,
        serial::{config::*, Event as SerialEvent, Rx, Serial, Tx},
        stm32::USART1,
    };
    // use systick_monotonic::*;

    // external task definitions
    use crate::uart::{ecdc_task, usart};

    // enphase specific imports
    use ecdc::{model_gen, Modelize}; // macros
    use ecdc::{Error, Item, Model, ModelInfo, Receiver};

    // use macros to generate data-model definitions
    model_gen!("datamodels/Data.xml");

    // collect the models for interfacing with the ECDC library
    #[derive(Modelize, Default)]
    pub struct Models {
        #[id(1)]
        pub data: Data::data,
    }

    const FREQ: u32 = 100_000_000;

    #[monotonic(binds = SysTick, default = true)]
    type SysMono = DwtSystick<FREQ>; // 100 MHz

    // Shared resources
    #[shared]
    struct Shared {
        models: Models,
    }

    // Local resources go here
    #[local]
    struct Local {
        tx: Tx<USART1>,
        rx: Rx<USART1>,
        led: PA5<Output<PushPull>>,
        ecdc_receiver: Receiver,
    }

    #[init(local = [adc_buf: [u16; 1] = [0u16; 1]])]
    fn init(mut cx: init::Context) -> (Shared, Local, init::Monotonics) {
        defmt::info!("init");

        cx.core.DCB.enable_trace();
        cx.core.DWT.enable_cycle_counter();

        // Set up the system clock.
        let rcc = cx.device.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(100.mhz()).freeze();
        let mono = DwtSystick::new(&mut cx.core.DCB, cx.core.DWT, cx.core.SYST, FREQ);

        // access the GPIO pins
        let gpioa = cx.device.GPIOA.split();
        let gpiob = cx.device.GPIOB.split();

        // Set up the LED. On the Nucleo-F411RE it's connected to pin PA5.
        let led = gpioa.pa5.into_push_pull_output();

        // Setup the UART
        let pin_tx = gpioa.pa9.into_alternate();
        let pin_rx = gpioa.pa10.into_alternate();
        let serial_config = Config {
            baudrate: 115_200.bps(),
            ..Config::default()
        };
        let mut serial =
            Serial::new(cx.device.USART1, (pin_tx, pin_rx), serial_config, clocks).unwrap();

        // listen for packets
        serial.listen(SerialEvent::Rxne);

        let (tx, rx) = serial.split();

        // create a new ecdc receiver
        let ecdc_receiver = Receiver::new();

        // initialize the data-model
        let mut models = Models::default();

        // spawn tasks
        blink::spawn_after(1.secs()).ok();

        // Setup resources
        (
            Shared { models },
            Local {
                tx,
                rx,
                led,
                ecdc_receiver,
            },
            init::Monotonics(mono),
        )
    }

    // Idle (CPU utilization monitor)
    #[idle()]
    fn idle(_cx: idle::Context) -> ! {
        defmt::info!("idle");
        loop {
            continue;
        }
    }

    #[task(local = [led])]
    fn blink(cx: blink::Context) {
        cx.local.led.toggle();
        blink::spawn_after(1.secs()).ok();
    }

    extern "Rust" {

        ////////////////////////////////////////////////////////////////////////////////////////////
        //                                  ECDC communication tasks
        ////////////////////////////////////////////////////////////////////////////////////////////
        #[task(binds = USART1, local = [rx])]
        fn usart(cx: usart::Context);

        #[task(capacity = 1, local = [tx, ecdc_receiver], shared=[models])]
        fn ecdc_task(cx: ecdc_task::Context, byte: u8);
    }
}
