//! A simple Driver for the Waveshare 5.83" (B) v2 E-Ink Display via SPI
//!
//! # References
//!
//! - [Datasheet](https://www.waveshare.com/5.83inch-e-Paper-B.htm)
//! - [Waveshare C driver](https://github.com/waveshare/e-Paper/blob/master/RaspberryPi_JetsonNano/c/lib/e-Paper/EPD_5in83b_V2.c)
//! - [Waveshare Python driver](https://github.com/waveshare/e-Paper/blob/master/RaspberryPi_JetsonNano/python/lib/waveshare_epd/epd5in83b_V2.py)

use embedded_hal::{
    blocking::{delay::*, spi::Write},
    digital::v2::{InputPin, OutputPin},
};

use crate::color::Color;
use crate::interface::DisplayInterface;
use crate::prelude::{TriColor, WaveshareDisplay, WaveshareThreeColorDisplay};
use crate::traits::{InternalWiAdditions, RefreshLut};

pub(crate) mod command;
use self::command::Command;

#[cfg(feature = "graphics")]
mod graphics;
#[cfg(feature = "graphics")]
pub use self::graphics::Display5in83;

/// Width of the display
pub const WIDTH: u32 = 648;
/// Height of the display
pub const HEIGHT: u32 = 480;
/// Default Background Color
pub const DEFAULT_BACKGROUND_COLOR: Color = Color::White;
const IS_BUSY_LOW: bool = true;
const NUM_DISPLAY_BITS: u32 = WIDTH * HEIGHT / 8;

/// Epd7in5 driver
///
pub struct Epd5in83<SPI, CS, BUSY, DC, RST, DELAY> {
    /// Connection Interface
    interface: DisplayInterface<SPI, CS, BUSY, DC, RST, DELAY>,
    /// Background Color
    color: Color,
}

impl<SPI, CS, BUSY, DC, RST, DELAY> InternalWiAdditions<SPI, CS, BUSY, DC, RST, DELAY>
    for Epd5in83<SPI, CS, BUSY, DC, RST, DELAY>
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
        self.interface.reset(delay, 10, 10);

        // Start the booster
        self.cmd_with_data(spi, Command::BoosterSoftStart, &[0x17, 0x17, 0x1e, 0x17])?;

        // Set the power settings: VGH=20V,VGL=-20V,VDH=15V,VDL=-15V
        self.cmd_with_data(spi, Command::PowerSetting, &[0x07, 0x07, 0x3F, 0x3F])?;

        // Power on
        self.command(spi, Command::PowerOn)?;
        delay.delay_ms(5);
        self.wait_until_idle();

        // Set the panel settings: BWROTP
        self.cmd_with_data(spi, Command::PanelSetting, &[0x0F])?;

        // Set the real resolution
        self.send_resolution(spi)?;

        // Disable dual SPI
        self.cmd_with_data(spi, Command::DualSPI, &[0x00])?;

        // Set Vcom and data interval
        self.cmd_with_data(spi, Command::VcomAndDataIntervalSetting, &[0x11, 0x07])?;

        // Set S2G and G2S non-overlap periods to 12 (default)
        self.cmd_with_data(spi, Command::TconSetting, &[0x22])?;

        self.wait_until_idle();
        Ok(())
    }
}

impl<SPI, CS, BUSY, DC, RST, DELAY> WaveshareThreeColorDisplay<SPI, CS, BUSY, DC, RST, DELAY>
    for Epd5in83<SPI, CS, BUSY, DC, RST, DELAY>
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
        self.update_chromatic_frame(spi, chromatic)?;
        Ok(())
    }

    fn update_achromatic_frame(&mut self, spi: &mut SPI, black: &[u8]) -> Result<(), SPI::Error> {
        self.wait_until_idle();
        self.cmd_with_data(spi, Command::DataStartTransmission1, black)?;
        Ok(())
    }

    fn update_chromatic_frame(
        &mut self,
        spi: &mut SPI,
        chromatic: &[u8],
    ) -> Result<(), SPI::Error> {
        self.wait_until_idle();
        self.cmd_with_data(spi, Command::DataStartTransmission2, chromatic)?;
        Ok(())
    }
}

impl<SPI, CS, BUSY, DC, RST, DELAY> WaveshareDisplay<SPI, CS, BUSY, DC, RST, DELAY>
    for Epd5in83<SPI, CS, BUSY, DC, RST, DELAY>
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

        let mut epd = Epd5in83 { interface, color };

        epd.init(spi, delay)?;

        Ok(epd)
    }

    fn sleep(&mut self, spi: &mut SPI, _delay: &mut DELAY) -> Result<(), SPI::Error> {
        self.wait_until_idle();
        self.command(spi, Command::PowerOff)?;
        self.wait_until_idle();
        self.cmd_with_data(spi, Command::DeepSleep, &[0xA5])?;
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
        self.update_achromatic_frame(spi, buffer)?;
        let color = self.color.get_byte_value();
        self.command(spi, Command::DataStartTransmission2)?;
        self.interface.data_x_times(spi, color, NUM_DISPLAY_BITS)?;
        Ok(())
    }

    fn update_partial_frame(
        &mut self,
        spi: &mut SPI,
        buffer: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<(), SPI::Error> {
        self.wait_until_idle();
        if buffer.len() as u32 != width / 8 * height {
            //TODO panic or error
        }

        let hrst_upper = (x / 8) as u8 >> 6;
        let hrst_lower = ((x / 8) << 3) as u8;
        let hred_upper = ((x + width) / 8) as u8 >> 6;
        let hred_lower = (((x + width) / 8) << 3) as u8 & 0b111;
        let vrst_upper = (y >> 8) as u8;
        let vrst_lower = y as u8;
        let vred_upper = ((y + height) >> 8) as u8;
        let vred_lower = (y + height) as u8;
        let pt_scan = 0x01; // Gates scan both inside and outside of the partial window. (default)

        self.command(spi, Command::PartialIn)?;
        self.command(spi, Command::PartialWindow)?;
        self.send_data(
            spi,
            &[
                hrst_upper, hrst_lower, hred_upper, hred_lower, vrst_upper, vrst_lower, vred_upper,
                vred_lower, pt_scan,
            ],
        )?;
        self.command(spi, Command::DataStartTransmission1)?;
        self.send_data(spi, buffer)?;

        let color = TriColor::Black.get_byte_value(); //We need it black, so red channel will be rendered transparent
        self.command(spi, Command::DataStartTransmission2)?;
        self.interface
            .data_x_times(spi, color, width * height / 8)?;

        self.command(spi, Command::DisplayRefresh)?;
        self.wait_until_idle();

        self.command(spi, Command::PartialOut)?;
        Ok(())
    }

    fn display_frame(&mut self, spi: &mut SPI, _delay: &mut DELAY) -> Result<(), SPI::Error> {
        self.command(spi, Command::DisplayRefresh)?;
        self.wait_until_idle();
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

        // The Waveshare controllers all implement clear using 0x33
        self.command(spi, Command::DataStartTransmission1)?;
        self.interface.data_x_times(spi, 0xFF, NUM_DISPLAY_BITS)?;

        self.command(spi, Command::DataStartTransmission2)?;
        self.interface.data_x_times(spi, 0x00, NUM_DISPLAY_BITS)?;

        Ok(())
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

impl<SPI, CS, BUSY, DC, RST, DELAY> Epd5in83<SPI, CS, BUSY, DC, RST, DELAY>
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
        assert_eq!(WIDTH, 648);
        assert_eq!(HEIGHT, 480);
        assert_eq!(DEFAULT_BACKGROUND_COLOR, Color::White);
    }
}
