//! Detection of a device stuck in download mode.

/// What the ROM bootloader prints when it boots into download mode.
const DOWNLOAD_MODE_MESSAGE: &[u8] = b"waiting for download";

/// Detects when the device connected over USB-Serial-JTAG boots into download
/// mode.
///
/// The strapping pins, and so the boot mode, are only latched on a chip reset,
/// i.e. a power-on or a reset through the EN pin (ESP32-C6 TRM, section 9.1 and
/// table 8.1-1). A reset over USB-Serial-JTAG is not one, so if the boot pin
/// was low at the last chip reset, e.g. because the BOOT button was held or the
/// device was reset into download mode through its UART port, every reset over
/// USB-Serial-JTAG boots into download mode again.
#[derive(Debug, Default)]
pub(crate) struct DownloadModeDetector {
    /// End of the previous input, to find the message across reads.
    tail: Vec<u8>,
    detected: bool,
}

impl DownloadModeDetector {
    /// Looks for the download mode message in the device output.
    ///
    /// Returns `true` the first time the message is found.
    pub(crate) fn feed(&mut self, data: &[u8]) -> bool {
        if self.detected {
            return false;
        }

        self.tail.extend_from_slice(data);
        if self
            .tail
            .windows(DOWNLOAD_MODE_MESSAGE.len())
            .any(|window| window == DOWNLOAD_MODE_MESSAGE)
        {
            self.detected = true;
            return true;
        }

        let keep = self.tail.len().min(DOWNLOAD_MODE_MESSAGE.len() - 1);
        self.tail.drain(..self.tail.len() - keep);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_message_once() {
        let mut detector = DownloadModeDetector::default();
        assert!(!detector.feed(b"ESP-ROM:esp32c6-20220919\r\n"));
        assert!(detector.feed(
            b"rst:0x15 (USB_UART_HPSYS),boot:0x4 (DOWNLOAD(USB/UART0/SDIO_FEI_FEO))\r\nwaiting for download\r\n"
        ));
        assert!(!detector.feed(b"waiting for download\r\n"));
    }

    #[test]
    fn detects_message_split_across_reads() {
        let mut detector = DownloadModeDetector::default();
        assert!(!detector.feed(b"Saved PC:0x408000f8\r\nwaiting for d"));
        assert!(!detector.feed(b""));
        assert!(detector.feed(b"ownload\r\n"));
    }

    #[test]
    fn ignores_application_output() {
        let mut detector = DownloadModeDetector::default();
        assert!(!detector.feed(b"I (99) boot: Loaded app from partition at offset 0x10000\r\n"));
        assert!(!detector.feed(b"INFO - Hello world!\r\n"));
    }
}
