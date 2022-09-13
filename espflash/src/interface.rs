use serialport::SerialPort;

/// Wrapper around SerialPort where platform-specific modifications can be implemented.
pub struct Interface {
    pub serial_port: Box<dyn SerialPort>,
}

impl Interface {
    pub fn write_data_terminal_ready(&mut self, pin_state: bool) -> serialport::Result<()> {
        self.serial_port.write_data_terminal_ready(pin_state)
    }

    pub fn write_request_to_send(&mut self, pin_state: bool) -> serialport::Result<()> {
        self.serial_port.write_request_to_send(pin_state)
    }

    pub fn into_serial(self) -> Box<dyn SerialPort> {
        self.serial_port
    }

    pub fn serial_port(&self) -> &dyn SerialPort {
        self.serial_port.as_ref()
    }

    pub fn serial_port_mut(&mut self) -> &mut dyn SerialPort {
        self.serial_port.as_mut()
    }
}
