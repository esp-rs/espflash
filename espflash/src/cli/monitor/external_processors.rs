#![allow(clippy::needless_doctest_main)]
//! External processor support
//!
//! Via the command line argument `--processors` you can instruct espflash to
//! run external executables to pre-process the logs received from the target.
//! Multiple processors are supported by separating them via `,`. Processors are
//! executed in the specified order.
//!
//! You can use full-qualified paths or run an executable which is already in
//! the search path.
//!
//! A processors reads from stdin and output to stdout. Be aware this runs
//! before further processing by espflash. i.e. addresses are not resolved and
//! when using `defmt` you will see encoded data.
//!
//! Additionally be aware that you might receive chunked data which is not
//! always split at valid UTF character boundaries.
//!
//! The executable will get the path of the ELF file as the first argument if
//! available.
//!
//! Example processor which turns some letters into uppercase
//! ```rust,no_run
//! use std::io::{stdin, stdout, Read, Write};
//!
//! fn main() {
//!     let args: Vec<String> = std::env::args().collect();
//!     println!("ELF file: {:?}", args[1]);
//!
//!     let mut buf = [0u8; 1024];
//!     loop {
//!         if let Ok(len) = stdin().read(&mut buf) {
//!             for b in &mut buf[..len] {
//!                 *b = if b"abdfeo".contains(b) {
//!                     b.to_ascii_uppercase()
//!                 } else {
//!                     *b
//!                 };
//!             }
//!
//!             stdout().write(&buf[..len]).unwrap();
//!             stdout().flush().unwrap();
//!         } else {
//!             // ignored
//!         }
//!     }
//! }
//! ```

use std::{
    fmt::Display,
    io::{Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Stdio},
    sync::mpsc,
};

use miette::Diagnostic;

/// Represents an error associated with a specific executable.
#[derive(Debug)]
pub struct Error {
    executable: String,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to launch '{}'", self.executable)
    }
}

impl std::error::Error for Error {}

impl Diagnostic for Error {}

#[derive(Debug)]
struct Processor {
    rx: mpsc::Receiver<u8>,
    stdin: ChildStdin,
    child: Child,
}

impl Processor {
    /// Creates a new processor from a child process.
    pub fn new(child: Child) -> Self {
        let mut child = child;
        let (tx, rx) = mpsc::channel::<u8>();

        let mut stdout = child.stdout.take().unwrap();
        let stdin = child.stdin.take().unwrap();

        std::thread::spawn(move || {
            let mut buffer = [0u8; 1024];
            loop {
                if let Ok(len) = stdout.read(&mut buffer) {
                    for b in &buffer[..len] {
                        if tx.send(*b).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Self { rx, stdin, child }
    }

    /// Tries to receive data from the processor.
    pub fn try_receive(&mut self) -> Vec<u8> {
        let mut res = Vec::new();
        while let Ok(b) = self.rx.try_recv() {
            res.push(b);
        }
        res
    }

    /// Sends data to the processor.
    pub fn send(&mut self, data: Vec<u8>) {
        let _ignored = self.stdin.write(&data).ok();
    }
}

impl Drop for Processor {
    fn drop(&mut self) {
        self.child.kill().unwrap();
    }
}

/// Represents a collection of external processors.
#[derive(Debug)]
pub struct ExternalProcessors {
    processors: Vec<Processor>,
}

impl ExternalProcessors {
    /// Creates a new collection of external processors.
    pub fn new(processors: Option<String>, elf: Option<PathBuf>) -> Result<Self, Error> {
        let mut args = Vec::new();

        if let Some(elf) = elf {
            args.push(elf.as_os_str().to_str().unwrap().to_string());
        };

        let mut spawned = Vec::new();
        if let Some(processors) = processors {
            for processor in processors.split(",") {
                let processor = std::process::Command::new(processor)
                    .args(args.clone())
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .map_err(|_| Error {
                        executable: processor.to_string(),
                    })?;
                spawned.push(Processor::new(processor));
            }
        }

        Ok(Self {
            processors: spawned,
        })
    }

    /// Processes input bytes through a series of processors, returning the
    /// final output.
    pub fn process(&mut self, read: &[u8]) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(read);

        for processor in &mut self.processors {
            processor.send(buffer);
            buffer = processor.try_receive();
        }

        buffer
    }
}
