//! A simple Driver for the Waveshare 7.5" E-Ink Display (V3) via SPI
//!
//! # References
//!
//! - [Datasheet](https://www.waveshare.com/wiki/7.5inch_e-Paper_HAT)
//! - [Waveshare C driver](https://github.com/waveshare/e-Paper/blob/702def0/RaspberryPi%26JetsonNano/c/lib/e-Paper/EPD_7in5_V2.c)
//! - [Waveshare Python driver](https://github.com/waveshare/e-Paper/blob/702def0/RaspberryPi%26JetsonNano/python/lib/waveshare_epd/epd7in5_V2.py)
//!

use embedded_hal::{
    blocking::{delay::*, spi::Write},
    digital::v2::{InputPin, OutputPin},
};

use crate::color::TriColor;
use crate::interface::DisplayInterface;
use crate::traits::{
    InternalWiAdditions, RefreshLut, WaveshareDisplay, WaveshareThreeColorDisplay,
};

pub(crate) mod command;
use self::command::Command;

#[cfg(feature = "graphics")]
mod graphics;
#[cfg(feature = "graphics")]
pub use self::graphics::Display7in5;

/// Width of the display
pub const WIDTH: u32 = 800;
/// Height of the display
pub const HEIGHT: u32 = 480;
/// Default Background Color
//pub const DEFAULT_BACKGROUND_COLOR: Color = Color::White;

pub const DEFAULT_BACKGROUND_COLOR: TriColor = TriColor::White;

/// Number of bits for b/w buffer and same for chromatic buffer
const NUM_DISPLAY_BITS: u32 = WIDTH * HEIGHT / 8;
const IS_BUSY_LOW: bool = true;

/// Epd7in5 (V3) driver
///
pub struct Epd7in5<SPI, CS, BUSY, DC, RST, DELAY> {
    /// Connection Interface
    interface: DisplayInterface<SPI, CS, BUSY, DC, RST, DELAY>,
    /// Background Color
    color: TriColor,
}

impl<SPI, CS, BUSY, DC, RST, DELAY> InternalWiAdditions<SPI, CS, BUSY, DC, RST, DELAY>
    for Epd7in5<SPI, CS, BUSY, DC, RST, DELAY>
where
    SPI: Write<u8>,
    CS: OutputPin,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayMs<u8>,
{
    fn init(&mut self, spi: &mut SPI, delay: &mut DELAY) -> Result<(), SPI::Error> {
        // Reset the device
        self.interface.reset(delay, 200, 4);

        // V2 procedure as described here:
        // https://github.com/waveshare/e-Paper/blob/master/RaspberryPi%26JetsonNano/python/lib/waveshare_epd/epd7in5bc_V2.py
        // and as per specs:
        // https://www.waveshare.com/w/upload/6/60/7.5inch_e-Paper_V2_Specification.pdf

        //self.cmd_with_data(spi, Command::BoosterSoftStart, &[0x17, 0x17, 0x27, 0x17])?;
        self.cmd_with_data(spi, Command::PowerSetting, &[0x07, 0x07, 0x3F, 0x3F])?;
        self.command(spi, Command::PowerOn)?;
        self.wait_until_idle(spi, delay)?;
        self.cmd_with_data(spi, Command::PanelSetting, &[0x0F])?;
        //self.cmd_with_data(spi, Command::PllControl, &[0x06])?;
        self.cmd_with_data(spi, Command::TconResolution, &[0x03, 0x20, 0x01, 0xE0])?;
        self.cmd_with_data(spi, Command::DualSpi, &[0x00])?;
        self.cmd_with_data(spi, Command::VcomAndDataIntervalSetting, &[0x11, 0x07])?;
        self.cmd_with_data(spi, Command::TconSetting, &[0x22])?;
        self.cmd_with_data(spi, Command::SpiFlashControl, &[0x00, 0x00, 0x00, 0x00])?;
        self.wait_until_idle(spi, delay)?;
        Ok(())
    }
}

impl<SPI, CS, BUSY, DC, RST, DELAY> WaveshareThreeColorDisplay<SPI, CS, BUSY, DC, RST, DELAY>
    for Epd7in5<SPI, CS, BUSY, DC, RST, DELAY>
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

    /// Update only the black/white data of the display.
    ///
    /// Finish by calling `update_chromatic_frame`.
    fn update_achromatic_frame(&mut self, spi: &mut SPI, black: &[u8]) -> Result<(), SPI::Error> {
        self.interface.cmd(spi, Command::DataStartTransmission1)?;
        self.interface.data(spi, black)?;
        Ok(())
    }

    /// Update only chromatic data of the display.
    ///
    /// This data takes precedence over the black/white data.
    fn update_chromatic_frame(
        &mut self,
        spi: &mut SPI,
        chromatic: &[u8],
    ) -> Result<(), SPI::Error> {
        self.interface.cmd(spi, Command::DataStartTransmission2)?;
        self.interface.data(spi, chromatic)?;

        self.wait_until_idle_raw()?;
        Ok(())
    }
}

impl<SPI, CS, BUSY, DC, RST, DELAY> WaveshareDisplay<SPI, CS, BUSY, DC, RST, DELAY>
    for Epd7in5<SPI, CS, BUSY, DC, RST, DELAY>
where
    SPI: Write<u8>,
    CS: OutputPin,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayMs<u8>,
{
    type DisplayColor = TriColor;
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

        let mut epd = Epd7in5 { interface, color };

        epd.init(spi, delay)?;

        Ok(epd)
    }

    fn wake_up(&mut self, spi: &mut SPI, delay: &mut DELAY) -> Result<(), SPI::Error> {
        self.init(spi, delay)
    }

    fn sleep(&mut self, spi: &mut SPI, delay: &mut DELAY) -> Result<(), SPI::Error> {
        self.wait_until_idle(spi, delay)?;
        self.command(spi, Command::PowerOff)?;
        self.wait_until_idle(spi, delay)?;
        self.cmd_with_data(spi, Command::DeepSleep, &[0xA5])?;
        Ok(())
    }

    fn update_frame(
        &mut self,
        spi: &mut SPI,
        buffer: &[u8],
        delay: &mut DELAY,
    ) -> Result<(), SPI::Error> {
        self.wait_until_idle(spi, delay)?;
        self.cmd_with_data(spi, Command::DataStartTransmission2, buffer)?;

        self.interface.data(spi, buffer)?;

        // Clear the chromatic layer
        let color = self.color.get_byte_value();

        self.interface.cmd(spi, Command::DataStartTransmission2)?;
        self.interface.data_x_times(spi, color, NUM_DISPLAY_BITS)?;

        self.wait_until_idle(spi, delay)?;

        Ok(())
    }

    fn update_partial_frame(
        &mut self,
        _spi: &mut SPI,
        _buffer: &[u8],
        _x: u32,
        _y: u32,
        _width: u32,
        _height: u32,
    ) -> Result<(), SPI::Error> {
        unimplemented!();
    }

    fn display_frame(&mut self, spi: &mut SPI, delay: &mut DELAY) -> Result<(), SPI::Error> {
        self.wait_until_idle(spi, delay)?;
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
        self.command(spi, Command::DisplayRefresh)?;
        Ok(())
    }

    fn clear_frame(&mut self, spi: &mut SPI, delay: &mut DELAY) -> Result<(), SPI::Error> {
        self.wait_until_idle(spi, delay)?;
        self.send_resolution(spi)?;

        self.command(spi, Command::DataStartTransmission1)?;
        self.interface.data_x_times(spi, 0x00, WIDTH * HEIGHT / 8)?;

        self.command(spi, Command::DataStartTransmission2)?;
        self.interface.data_x_times(spi, 0x00, WIDTH * HEIGHT / 8)?;

        self.command(spi, Command::DisplayRefresh)?;
        Ok(())
    }

    fn set_background_color(&mut self, color: TriColor) {
        self.color = color;
    }

    fn background_color(&self) -> &TriColor {
        &self.color
    }

    fn width(&self) -> u32 {
        WIDTH
    }

    fn height(&self) -> u32 {
        HEIGHT
    }

    fn set_lut(
        &mut self,
        _spi: &mut SPI,
        _refresh_rate: Option<RefreshLut>,
    ) -> Result<(), SPI::Error> {
        unimplemented!();
    }

    fn is_busy(&self) -> bool {
        self.interface.is_busy(IS_BUSY_LOW)
    }
}

impl<SPI, CS, BUSY, DC, RST, DELAY> Epd7in5<SPI, CS, BUSY, DC, RST, DELAY>
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

    fn wait_until_idle(&mut self, spi: &mut SPI, delay: &mut DELAY) -> Result<(), SPI::Error> {
        while self.interface.is_busy(IS_BUSY_LOW) {
            self.interface.cmd(spi, Command::GetStatus)?;
            delay.delay_ms(20);
        }
        Ok(())
    }

    fn wait_until_idle_raw(&mut self) -> Result<(), SPI::Error> {
        self.interface.wait_until_idle(IS_BUSY_LOW);
        Ok(())
    }

    fn send_resolution(&mut self, spi: &mut SPI) -> Result<(), SPI::Error> {
        let w = self.width();
        let h = self.height();

        self.command(spi, Command::TconResolution)?;
        self.send_data(spi, &[(w >> 8) as u8])?;
        self.send_data(spi, &[w as u8])?;
        self.send_data(spi, &[(h >> 8) as u8])?;
        self.send_data(spi, &[h as u8])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epd_size() {
        assert_eq!(WIDTH, 800);
        assert_eq!(HEIGHT, 480);
        assert_eq!(DEFAULT_BACKGROUND_COLOR, TriColor::White);
    }
}
