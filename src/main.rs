
#![no_std]
#![no_main]

use panic_probe as _;
use defmt_rtt as _;

use embassy_executor::Spawner;

use embassy_stm32::gpio::{
    Input,
    Level,
    Output,
    Pull,
    Speed,
};

use embassy_stm32::i2c::{
    Config as I2cConfig,
    I2c,
};

use embassy_time::{
    Duration,
    Timer,
};

use embedded_hal::i2c::I2c as I2cTrait;


const MAX_LEVEL: usize = 20;
const OLED_ADDR: u8 = 0x3C;
const COL_OFFSET: u8 = 2;


fn color_frequency(number: usize) -> u32 {
    match number {
        0 => 523,
        1 => 659,
        2 => 784,
        3 => 988,
        _ => 440,
    }
}


fn next_random(seed: &mut u32) -> usize {
    *seed = seed
        .wrapping_mul(1_664_525)
        .wrapping_add(1_013_904_223);

    ((*seed >> 16) % 4) as usize
}


fn all_leds_off(
    white: &mut Output<'_>,
    blue: &mut Output<'_>,
    yellow: &mut Output<'_>,
    red: &mut Output<'_>,
) {
    white.set_low();
    blue.set_low();
    yellow.set_low();
    red.set_low();
}


fn led_on(
    number: usize,
    white: &mut Output<'_>,
    blue: &mut Output<'_>,
    yellow: &mut Output<'_>,
    red: &mut Output<'_>,
) {
    all_leds_off(white, blue, yellow, red);

    match number {
        0 => white.set_high(),
        1 => blue.set_high(),
        2 => yellow.set_high(),
        3 => red.set_high(),
        _ => {}
    }
}


async fn tone(
    buzzer: &mut Output<'_>,
    frequency_hz: u32,
    duration_ms: u64,
) {
    let half_period_us = 1_000_000 / (frequency_hz as u64 * 2);
    let total_cycles = (duration_ms * frequency_hz as u64) / 1000;

    for _ in 0..total_cycles {
        buzzer.set_high();
        Timer::after(Duration::from_micros(half_period_us)).await;

        buzzer.set_low();
        Timer::after(Duration::from_micros(half_period_us)).await;
    }
}


async fn show_color(
    number: usize,
    white: &mut Output<'_>,
    blue: &mut Output<'_>,
    yellow: &mut Output<'_>,
    red: &mut Output<'_>,
    buzzer: &mut Output<'_>,
) {
    led_on(number, white, blue, yellow, red);

    tone(
        buzzer,
        color_frequency(number),
        350,
    )
    .await;

    all_leds_off(white, blue, yellow, red);

    Timer::after(Duration::from_millis(180)).await;
}


fn read_button(
    white: &Input<'_>,
    blue: &Input<'_>,
    yellow: &Input<'_>,
    red: &Input<'_>,
) -> Option<usize> {
    if white.is_low() {
        Some(0)
    } else if blue.is_low() {
        Some(1)
    } else if yellow.is_low() {
        Some(2)
    } else if red.is_low() {
        Some(3)
    } else {
        None
    }
}


fn oled_command<I: I2cTrait>(
    i2c: &mut I,
    command: u8,
) {
    let data = [0x00, command];
    let _ = i2c.write(OLED_ADDR, &data);
}


fn oled_data<I: I2cTrait>(
    i2c: &mut I,
    data: &[u8],
) {
    let mut buffer = [0u8; 129];

    buffer[0] = 0x40;

    let mut i = 0;

    while i < data.len() && i < 128 {
        buffer[i + 1] = data[i];
        i += 1;
    }

    let _ = i2c.write(
        OLED_ADDR,
        &buffer[..i + 1],
    );
}


fn oled_init<I: I2cTrait>(
    i2c: &mut I,
) {
    oled_command(i2c, 0xAE);

    oled_command(i2c, 0xD5);
    oled_command(i2c, 0x80);

    oled_command(i2c, 0xA8);
    oled_command(i2c, 0x3F);

    oled_command(i2c, 0xD3);
    oled_command(i2c, 0x00);

    oled_command(i2c, 0x40);

    oled_command(i2c, 0x8D);
    oled_command(i2c, 0x14);

    oled_command(i2c, 0x20);
    oled_command(i2c, 0x02);

    oled_command(i2c, 0xA1);
    oled_command(i2c, 0xC8);

    oled_command(i2c, 0xDA);
    oled_command(i2c, 0x12);

    oled_command(i2c, 0x81);
    oled_command(i2c, 0xCF);

    oled_command(i2c, 0xD9);
    oled_command(i2c, 0xF1);

    oled_command(i2c, 0xDB);
    oled_command(i2c, 0x40);

    oled_command(i2c, 0xA4);
    oled_command(i2c, 0xA6);

    oled_command(i2c, 0xAF);
}


fn oled_set_position<I: I2cTrait>(
    i2c: &mut I,
    page: u8,
    column: u8,
) {
    let column = column + COL_OFFSET;

    oled_command(i2c, 0xB0 + page);
    oled_command(i2c, column & 0x0F);
    oled_command(i2c, 0x10 + ((column >> 4) & 0x0F));
}


fn oled_clear<I: I2cTrait>(
    i2c: &mut I,
) {
    let empty = [0u8; 128];

    for page in 0..8 {
        oled_set_position(i2c, page, 0);
        oled_data(i2c, &empty);
    }
}


fn font(ch: u8) -> [u8; 5] {
    match ch {
        b'0' => [0x3E, 0x51, 0x49, 0x45, 0x3E],
        b'1' => [0x00, 0x42, 0x7F, 0x40, 0x00],
        b'2' => [0x42, 0x61, 0x51, 0x49, 0x46],
        b'3' => [0x21, 0x41, 0x45, 0x4B, 0x31],
        b'4' => [0x18, 0x14, 0x12, 0x7F, 0x10],
        b'5' => [0x27, 0x45, 0x45, 0x45, 0x39],
        b'6' => [0x3C, 0x4A, 0x49, 0x49, 0x30],
        b'7' => [0x01, 0x71, 0x09, 0x05, 0x03],
        b'8' => [0x36, 0x49, 0x49, 0x49, 0x36],
        b'9' => [0x06, 0x49, 0x49, 0x29, 0x1E],

        b'A' => [0x7E, 0x11, 0x11, 0x11, 0x7E],
        b'C' => [0x3E, 0x41, 0x41, 0x41, 0x22],
        b'E' => [0x7F, 0x49, 0x49, 0x49, 0x41],
        b'I' => [0x00, 0x41, 0x7F, 0x41, 0x00],
        b'N' => [0x7F, 0x04, 0x08, 0x10, 0x7F],
        b'O' => [0x3E, 0x41, 0x41, 0x41, 0x3E],
        b'P' => [0x7F, 0x09, 0x09, 0x09, 0x06],
        b'R' => [0x7F, 0x09, 0x19, 0x29, 0x46],
        b'S' => [0x46, 0x49, 0x49, 0x49, 0x31],
        b'T' => [0x01, 0x01, 0x7F, 0x01, 0x01],
        b'U' => [0x3F, 0x40, 0x40, 0x40, 0x3F],
        b'W' => [0x3F, 0x40, 0x38, 0x40, 0x3F],
        b'Y' => [0x07, 0x08, 0x70, 0x08, 0x07],

        b':' => [0x00, 0x36, 0x36, 0x00, 0x00],
        b' ' => [0x00, 0x00, 0x00, 0x00, 0x00],

        _ => [0x00, 0x00, 0x00, 0x00, 0x00],
    }
}


fn oled_char<I: I2cTrait>(
    i2c: &mut I,
    ch: u8,
) {
    let character = font(ch);

    let data = [
        character[0],
        character[1],
        character[2],
        character[3],
        character[4],
        0x00,
    ];

    oled_data(i2c, &data);
}


fn oled_text<I: I2cTrait>(
    i2c: &mut I,
    page: u8,
    column: u8,
    text: &str,
) {
    let mut column = column;

    for ch in text.bytes() {
        oled_set_position(i2c, page, column);
        oled_char(i2c, ch);

        column += 6;
    }
}


fn show_press_start<I: I2cTrait>(
    i2c: &mut I,
) {
    oled_clear(i2c);
    oled_text(i2c, 3, 31, "PRESS START");
}


fn show_score<I: I2cTrait>(
    i2c: &mut I,
    score: usize,
    win: bool,
) {
    oled_clear(i2c);

    if win {
        oled_text(i2c, 2, 42, "YOU WIN");
    }

    oled_text(i2c, 4, 36, "SCORE:");

    let tens = b'0' + ((score / 10) as u8);
    let units = b'0' + ((score % 10) as u8);

    oled_set_position(i2c, 4, 78);
    oled_char(i2c, tens);

    oled_set_position(i2c, 4, 84);
    oled_char(i2c, units);
}


#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    // LEDs
    let mut led_white = Output::new(p.PB4, Level::Low, Speed::Low);
    let mut led_blue = Output::new(p.PB10, Level::Low, Speed::Low);
    let mut led_yellow = Output::new(p.PC6, Level::Low, Speed::Low);
    let mut led_red = Output::new(p.PC9, Level::Low, Speed::Low);

    // Buttons
    let start_button = Input::new(p.PA0, Pull::Up);
    let button_white = Input::new(p.PA1, Pull::Up);
    let button_blue = Input::new(p.PA4, Pull::Up);
    let button_yellow = Input::new(p.PB0, Pull::Up);
    let button_red = Input::new(p.PC1, Pull::Up);

    // Buzzer
    let mut buzzer = Output::new(p.PB3, Level::Low, Speed::Low);

    // OLED
    let mut i2c = I2c::new_blocking(
        p.I2C1,
        p.PB6,
        p.PB7,
        I2cConfig::default(),
    );

    Timer::after(Duration::from_millis(200)).await;

    oled_init(&mut i2c);

    Timer::after(Duration::from_millis(50)).await;

    oled_clear(&mut i2c);

    let mut sequence = [0usize; MAX_LEVEL];
    let mut seed: u32 = 1;
    let mut idle_led = 0usize;

    loop {
        show_press_start(&mut i2c);

        // Idle
        loop {
            led_on(
                idle_led,
                &mut led_white,
                &mut led_blue,
                &mut led_yellow,
                &mut led_red,
            );

            idle_led += 1;

            if idle_led >= 4 {
                idle_led = 0;
            }

            seed = seed.wrapping_add(1);

            if start_button.is_low() {
                break;
            }

            Timer::after(Duration::from_millis(200)).await;
        }

        Timer::after(Duration::from_millis(200)).await;

        while start_button.is_low() {
            Timer::after(Duration::from_millis(10)).await;
        }

        all_leds_off(
            &mut led_white,
            &mut led_blue,
            &mut led_yellow,
            &mut led_red,
        );

        oled_clear(&mut i2c);

        let mut score = 0usize;
        let mut game_over = false;

        // Levels
        for level in 0..MAX_LEVEL {
            sequence[level] = next_random(&mut seed);

            Timer::after(Duration::from_millis(500)).await;

            // Show sequence
            for i in 0..=level {
                show_color(
                    sequence[i],
                    &mut led_white,
                    &mut led_blue,
                    &mut led_yellow,
                    &mut led_red,
                    &mut buzzer,
                )
                .await;
            }

            // Player input
            for i in 0..=level {
                let pressed;

                loop {
                    if let Some(button) = read_button(
                        &button_white,
                        &button_blue,
                        &button_yellow,
                        &button_red,
                    ) {
                        pressed = button;
                        break;
                    }

                    Timer::after(Duration::from_millis(10)).await;
                }

                led_on(
                    pressed,
                    &mut led_white,
                    &mut led_blue,
                    &mut led_yellow,
                    &mut led_red,
                );

                tone(
                    &mut buzzer,
                    color_frequency(pressed),
                    250,
                )
                .await;

                all_leds_off(
                    &mut led_white,
                    &mut led_blue,
                    &mut led_yellow,
                    &mut led_red,
                );

                while read_button(
                    &button_white,
                    &button_blue,
                    &button_yellow,
                    &button_red,
                )
                .is_some()
                {
                    Timer::after(Duration::from_millis(10)).await;
                }

                if pressed != sequence[i] {
                    game_over = true;
                    break;
                }
            }

            if game_over {
                break;
            }

            score += 1;

            tone(
                &mut buzzer,
                1200,
                100,
            )
            .await;

            Timer::after(Duration::from_millis(400)).await;
        }

        if score == MAX_LEVEL {
            show_score(
                &mut i2c,
                score,
                true,
            );

            for _ in 0..4 {
                led_white.set_high();
                led_blue.set_high();
                led_yellow.set_high();
                led_red.set_high();

                tone(
                    &mut buzzer,
                    1200,
                    100,
                )
                .await;

                all_leds_off(
                    &mut led_white,
                    &mut led_blue,
                    &mut led_yellow,
                    &mut led_red,
                );

                Timer::after(Duration::from_millis(100)).await;
            }
        } else {
            show_score(
                &mut i2c,
                score,
                false,
            );

            for _ in 0..3 {
                led_white.set_high();
                led_blue.set_high();
                led_yellow.set_high();
                led_red.set_high();

                tone(
                    &mut buzzer,
                    300,
                    150,
                )
                .await;

                all_leds_off(
                    &mut led_white,
                    &mut led_blue,
                    &mut led_yellow,
                    &mut led_red,
                );

                Timer::after(Duration::from_millis(100)).await;
            }
        }

        Timer::after(Duration::from_millis(2000)).await;

        all_leds_off(
            &mut led_white,
            &mut led_blue,
            &mut led_yellow,
            &mut led_red,
        );

        idle_led = 0;
    }
}
