//! A simple Driver for the Waveshare 1.54" (B) E-Ink Display via SPI

use embedded_hal::{
    blocking::{delay::*, spi::Write},
    digital::v2::*,
};

use crate::interface::DisplayInterface;
use crate::traits::{
    InternalWiAdditions, RefreshLut, WaveshareDisplay, WaveshareThreeColorDisplay,
};

//The Lookup Tables for the Display
mod constants;
use crate::epd1in54b::constants::*;

/// Width of epd1in54 in pixels
pub const WIDTH: u32 = 200;
/// Height of epd1in54 in pixels
pub const HEIGHT: u32 = 200;
/// Default Background Color (white)
pub const DEFAULT_BACKGROUND_COLOR: Color = Color::White;
const IS_BUSY_LOW: bool = true;

use crate::color::Color;

pub(crate) mod command;
use self::command::Command;

#[cfg(feature = "graphics")]
mod graphics;
#[cfg(feature = "graphics")]
pub use self::graphics::Display1in54b;

/// Epd1in54b driver
pub struct Epd1in54b<SPI, CS, BUSY, DC, RST, DELAY> {
    interface: DisplayInterface<SPI, CS, BUSY, DC, RST, DELAY>,
    color: Color,
}

impl<SPI, CS, BUSY, DC, RST, DELAY> InternalWiAdditions<SPI, CS, BUSY, DC, RST, DELAY>
    for Epd1in54b<SPI, CS, BUSY, DC, RST, DELAY>
where
    SPI: Write<u8>,
    CS: OutputPin,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayMs<u8>,
{
    fn init(&mut self, spi: &mut SPI, delay: &mut DELAY) -> Result<(), SPI::Error> {
        self.interface.reset(delay, 10, 10);

        // set the power settings
        self.interface
            .cmd_with_data(spi, Command::PowerSetting, &[0x07, 0x00, 0x08, 0x00])?;

        // start the booster
        self.interface
            .cmd_with_data(spi, Command::BoosterSoftStart, &[0x07, 0x07, 0x07])?;

        // power on
        self.command(spi, Command::PowerOn)?;
        delay.delay_ms(5);
        self.wait_until_idle();

        // set the panel settings
        self.cmd_with_data(spi, Command::PanelSetting, &[0xCF])?;

        self.cmd_with_data(spi, Command::VcomAndDataIntervalSetting, &[0x37])?;

        // PLL
        self.cmd_with_data(spi, Command::PllControl, &[0x39])?;

        // set resolution
        self.send_resolution(spi)?;

        self.cmd_with_data(spi, Command::VcmDcSetting, &[0x0E])?;

        self.set_lut(spi, None)?;

        self.wait_until_idle();

        Ok(())
    }
}

impl<SPI, CS, BUSY, DC, RST, DELAY> WaveshareThreeColorDisplay<SPI, CS, BUSY, DC, RST, DELAY>
    for Epd1in54b<SPI, CS, BUSY, DC, RST, DELAY>
where
    SPI: Write<u8>,
    CS: OutputPin,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayMs<u8>,
{
    fn update_color_frame(
        &mut self,
        spi: &mut SPI,
        black: &[u8],
        chromatic: &[u8],
    ) -> Result<(), SPI::Error> {
        self.update_achromatic_frame(spi, black)?;
        self.update_chromatic_frame(spi, chromatic)
    }

    fn update_achromatic_frame(&mut self, spi: &mut SPI, black: &[u8]) -> Result<(), SPI::Error> {
        self.wait_until_idle();
        self.send_resolution(spi)?;

        self.interface.cmd(spi, Command::DataStartTransmission1)?;

        for b in black {
            let expanded = expand_bits(*b);
            self.interface.data(spi, &expanded)?;
        }
        Ok(())
    }

    fn update_chromatic_frame(
        &mut self,
        spi: &mut SPI,
        chromatic: &[u8],
    ) -> Result<(), SPI::Error> {
        self.interface.cmd(spi, Command::DataStartTransmission2)?;
        self.interface.data(spi, chromatic)?;
        Ok(())
    }
}

impl<SPI, CS, BUSY, DC, RST, DELAY> WaveshareDisplay<SPI, CS, BUSY, DC, RST, DELAY>
    for Epd1in54b<SPI, CS, BUSY, DC, RST, DELAY>
where
    SPI: Write<u8>,
    CS: OutputPin,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayMs<u8>,
{
    type DisplayColor = Color;
    fn new(
        spi: &mut SPI,
        cs: CS,
        busy: BUSY,
        dc: DC,
        rst: RST,
        delay: &mut DELAY,
    ) -> Result<Self, SPI::Error> {
        let interface = DisplayInterface::new(cs, busy, dc, rst);
        let color = DEFAULT_BACKGROUND_COLOR;

        let mut epd = Epd1in54b { interface, color };

        epd.init(spi, delay)?;

        Ok(epd)
    }

    fn sleep(&mut self, spi: &mut SPI, _delay: &mut DELAY) -> Result<(), SPI::Error> {
        self.wait_until_idle();
        self.interface
            .cmd_with_data(spi, Command::VcomAndDataIntervalSetting, &[0x17])?; //border floating

        self.interface
            .cmd_with_data(spi, Command::VcmDcSetting, &[0x00])?; // Vcom to 0V

        self.interface
            .cmd_with_data(spi, Command::PowerSetting, &[0x02, 0x00, 0x00, 0x00])?; //VG&VS to 0V fast

        self.wait_until_idle();

        //NOTE: The example code has a 1s delay here

        self.command(spi, Command::PowerOff)?;

        Ok(())
    }

    fn wake_up(&mut self, spi: &mut SPI, delay: &mut DELAY) -> Result<(), SPI::Error> {
        self.init(spi, delay)
    }

    fn set_background_color(&mut self, color: Color) {
        self.color = color;
    }

    fn background_color(&self) -> &Color {
        &self.color
    }

    fn width(&self) -> u32 {
        WIDTH
    }

    fn height(&self) -> u32 {
        HEIGHT
    }

    fn update_frame(
        &mut self,
        spi: &mut SPI,
        buffer: &[u8],
        _delay: &mut DELAY,
    ) -> Result<(), SPI::Error> {
        self.wait_until_idle();
        self.send_resolution(spi)?;

        self.interface.cmd(spi, Command::DataStartTransmission1)?;

        for b in buffer {
            // Two bits per pixel
            let expanded = expand_bits(*b);
            self.interface.data(spi, &expanded)?;
        }

        //NOTE: Example code has a delay here

        // Clear the read layer
        let color = self.color.get_byte_value();
        let nbits = WIDTH * (HEIGHT / 8);

        self.interface.cmd(spi, Command::DataStartTransmission2)?;
        self.interface.data_x_times(spi, color, nbits)?;

        //NOTE: Example code has a delay here
        Ok(())
    }

    #[allow(unused)]
    fn update_partial_frame(
        &mut self,
        spi: &mut SPI,
        buffer: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<(), SPI::Error> {
        unimplemented!()
    }

    fn display_frame(&mut self, spi: &mut SPI, _delay: &mut DELAY) -> Result<(), SPI::Error> {
        self.wait_until_idle();
        self.command(spi, Command::DisplayRefresh)?;
        Ok(())
    }

    fn update_and_display_frame(
        &mut self,
        spi: &mut SPI,
        buffer: &[u8],
        delay: &mut DELAY,
    ) -> Result<(), SPI::Error> {
        self.update_frame(spi, buffer, delay)?;
        self.display_frame(spi, delay)?;
        Ok(())
    }

    fn clear_frame(&mut self, spi: &mut SPI, _delay: &mut DELAY) -> Result<(), SPI::Error> {
        self.wait_until_idle();
        self.send_resolution(spi)?;

        let color = DEFAULT_BACKGROUND_COLOR.get_byte_value();

        // Clear the black
        self.interface.cmd(spi, Command::DataStartTransmission1)?;

        // Uses 2 bits per pixel
        self.interface
            .data_x_times(spi, color, 2 * (WIDTH * HEIGHT / 8))?;

        // Clear the red
        self.interface.cmd(spi, Command::DataStartTransmission2)?;
        self.interface
            .data_x_times(spi, color, WIDTH * HEIGHT / 8)?;
        Ok(())
    }

    fn set_lut(
        &mut self,
        spi: &mut SPI,
        _refresh_rate: Option<RefreshLut>,
    ) -> Result<(), SPI::Error> {
        self.interface
            .cmd_with_data(spi, Command::LutForVcom, LUT_VCOM0)?;
        self.interface
            .cmd_with_data(spi, Command::LutWhiteToWhite, LUT_WHITE_TO_WHITE)?;
        self.interface
            .cmd_with_data(spi, Command::LutBlackToWhite, LUT_BLACK_TO_WHITE)?;
        self.interface.cmd_with_data(spi, Command::LutG0, LUT_G1)?;
        self.interface.cmd_with_data(spi, Command::LutG1, LUT_G2)?;
        self.interface
            .cmd_with_data(spi, Command::LutRedVcom, LUT_RED_VCOM)?;
        self.interface
            .cmd_with_data(spi, Command::LutRed0, LUT_RED0)?;
        self.interface
            .cmd_with_data(spi, Command::LutRed1, LUT_RED1)?;

        Ok(())
    }

    fn is_busy(&self) -> bool {
        self.interface.is_busy(IS_BUSY_LOW)
    }
}

impl<SPI, CS, BUSY, DC, RST, DELAY> Epd1in54b<SPI, CS, BUSY, DC, RST, DELAY>
where
    SPI: Write<u8>,
    CS: OutputPin,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayMs<u8>,
{
    fn command(&mut self, spi: &mut SPI, command: Command) -> Result<(), SPI::Error> {
        self.interface.cmd(spi, command)
    }

    fn send_data(&mut self, spi: &mut SPI, data: &[u8]) -> Result<(), SPI::Error> {
        self.interface.data(spi, data)
    }

    fn cmd_with_data(
        &mut self,
        spi: &mut SPI,
        command: Command,
        data: &[u8],
    ) -> Result<(), SPI::Error> {
        self.interface.cmd_with_data(spi, command, data)
    }

    fn wait_until_idle(&mut self) {
        self.interface.wait_until_idle(IS_BUSY_LOW);
    }

    fn send_resolution(&mut self, spi: &mut SPI) -> Result<(), SPI::Error> {
        let w = self.width();
        let h = self.height();

        self.command(spi, Command::ResolutionSetting)?;

        self.send_data(spi, &[w as u8])?;
        self.send_data(spi, &[(h >> 8) as u8])?;
        self.send_data(spi, &[h as u8])
    }
}

fn expand_bits(bits: u8) -> [u8; 2] {
    let mut x = bits as u16;

    x = (x | (x << 4)) & 0x0F0F;
    x = (x | (x << 2)) & 0x3333;
    x = (x | (x << 1)) & 0x5555;
    x = x | (x << 1);

    [(x >> 8) as u8, (x & 0xFF) as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epd_size() {
        assert_eq!(WIDTH, 200);
        assert_eq!(HEIGHT, 200);
        assert_eq!(DEFAULT_BACKGROUND_COLOR, Color::White);
    }
}
