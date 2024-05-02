//! Combination between
//! * https://github.com/embassy-rs/embassy/blob/main/examples/rp/src/bin/blinky.rs
//! * https://github.com/embassy-rs/embassy/blob/main/examples/rp/src/bin/pio_ws2812.rs

#![no_std]
#![no_main]

use defmt::info;

use embassy_executor::Spawner;
use embassy_rp::gpio::{AnyPin, Level, Output, Pin};
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_time::{Duration, Ticker, Timer};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::{Channel as SyncChannel, Receiver};

use ws2812;

use smart_leds::RGB8;
use {defmt_rtt as _, panic_probe as _};

enum SleepLonger { Yes }
static CHANNEL1: SyncChannel<ThreadModeRawMutex, SleepLonger, 64> = SyncChannel::new();
static CHANNEL2: SyncChannel<ThreadModeRawMutex, SleepLonger, 64> = SyncChannel::new();
static CHANNEL3: SyncChannel<ThreadModeRawMutex, SleepLonger, 64> = SyncChannel::new();
static CHANNEL4: SyncChannel<ThreadModeRawMutex, SleepLonger, 64> = SyncChannel::new();

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

// ================================================================================

// Input a value 0 to 255 to get a color value
// The colours are a transition r - g - b - back to r.
fn wheel(mut wheel_pos: u8) -> RGB8 {
    wheel_pos = 255 - wheel_pos;
    if wheel_pos < 85 {
        return (255 - wheel_pos * 3, 0, wheel_pos * 3).into();
    }
    if wheel_pos < 170 {
        wheel_pos -= 85;
        return (0, wheel_pos * 3, 255 - wheel_pos * 3).into();
    }
    wheel_pos -= 170;
    (wheel_pos * 3, 255 - wheel_pos * 3, 0).into()
}

// A pool size of 4 means you can spawn four instances of this task.
// One for each status LED.
#[embassy_executor::task(pool_size = 4)]
async fn led_blink(control: Receiver<'static, ThreadModeRawMutex, SleepLonger, 64>, pin: AnyPin) {
    let mut led = Output::new(pin, Level::Low);

    loop {
	led.set_high();
	Timer::after_millis(150).await;

	led.set_low();

	match control.try_receive() {
	    core::prelude::v1::Ok(SleepLonger::Yes) => {
		info!("SleepLonger = Yes received, sleeping 3s");
		Timer::after_secs(3).await;
	    },
	    _ => {
		info!("Nothing received, sleeping 150ms");
		Timer::after_millis(150).await;
	    }
	}
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Start");

    let p = embassy_rp::init(Default::default());

    // =====
    // Initialize the NeoPixel LED.
    let Pio { mut common, sm0, .. } = Pio::new(p.PIO0, Irqs);
    let mut ws2812 = ws2812::Ws2812::new(&mut common, sm0, p.DMA_CH0, p.PIN_15);

    // This is the number of leds in the string. Helpfully, the sparkfun thing plus and adafruit
    // feather boards for the 2040 both have one built in.
    const NUM_LEDS: usize = 1;
    let mut data = [RGB8::default(); NUM_LEDS];

    // =====
    // Spawn a LED blinker task, one per LED.
    spawner.spawn(led_blink(CHANNEL1.receiver(), p.PIN_6.degrade())).unwrap();
    Timer::after_secs(1).await;
    spawner.spawn(led_blink(CHANNEL2.receiver(), p.PIN_7.degrade())).unwrap();
    Timer::after_secs(1).await;
    spawner.spawn(led_blink(CHANNEL3.receiver(), p.PIN_8.degrade())).unwrap();
    Timer::after_secs(1).await;
    spawner.spawn(led_blink(CHANNEL4.receiver(), p.PIN_9.degrade())).unwrap();
    Timer::after_secs(1).await;

    // Loop forever making RGB values and pushing them out to the WS2812.
    let mut ticker = Ticker::every(Duration::from_millis(10));
    loop {
	info!("NeoPixel off");
	ws2812.write(&[(0,0,0).into()]).await;

	// Tell the LEDs to sleep longer.
	CHANNEL1.send(SleepLonger::Yes).await;
	Timer::after_secs(1).await;
	CHANNEL2.send(SleepLonger::Yes).await;
	Timer::after_secs(1).await;
	CHANNEL3.send(SleepLonger::Yes).await;
	Timer::after_secs(1).await;
	CHANNEL4.send(SleepLonger::Yes).await;

	// =====
	
	// BLUE
	ws2812.write(&[(0,0,255).into()]).await;
	Timer::after_secs(1).await;
	ws2812.write(&[(0,0,0).into()]).await;
	Timer::after_secs(1).await;

	// GREEN
	ws2812.write(&[(255,0,0).into()]).await;
	Timer::after_secs(1).await;
	ws2812.write(&[(0,0,0).into()]).await;
	Timer::after_secs(1).await;

	// ORANGE
	ws2812.write(&[(130,255,0).into()]).await;
	Timer::after_secs(1).await;
	ws2812.write(&[(0,0,0).into()]).await;
	Timer::after_secs(1).await;

	// RED
	ws2812.write(&[(0,255,0).into()]).await;
	Timer::after_secs(1).await;
	ws2812.write(&[(0,0,0).into()]).await;
	Timer::after_secs(1).await;

	// WHITE
	ws2812.write(&[(255,255,255).into()]).await;
	Timer::after_secs(1).await;
	ws2812.write(&[(0,0,0).into()]).await;
	Timer::after_secs(1).await;

	// =====

        for j in 0..(256 * 5) {
            info!("New Colors:");
            for i in 0..NUM_LEDS {
                data[i] = wheel((((i * 256) as u16 / NUM_LEDS as u16 + j as u16) & 255) as u8);
                info!("R: {} G: {} B: {}", data[i].r, data[i].g, data[i].b);
            }
            ws2812.write(&data).await;

            ticker.next().await;
        }

        Timer::after_secs(1).await;
    }
}
