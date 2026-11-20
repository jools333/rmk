use embassy_nrf::gpio::{Flex, OutputDrive, Pull};

use crate::driver::flex_pin::FlexPin;

impl<'d> FlexPin for Flex<'d> {
    fn set_as_input(&mut self) {
        self.set_as_input(Pull::Up);
    }

    fn set_as_output(&mut self) {
        self.set_as_output(OutputDrive::Standard);
    }
}
