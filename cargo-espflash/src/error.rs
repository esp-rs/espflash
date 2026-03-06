#![allow(unused_assignments)]

use std::{
    fmt::{Display, Formatter},
    iter::once,
};

use espflash::target::Chip;
use miette::{Diagnostic, LabeledSpan, SourceCode};
use thiserror::Error;

/// Error type returned by `cargo-espflash`.
#[derive(Debug, Diagnostic, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Multiple build artifacts found")]
    #[diagnostic(
        code(cargo_espflash::multiple_artifacts),
        help("Please specify which artifact to flash using `--bin`")
    )]
    MultipleArtifacts,

    #[error("No executable artifact found")]
    #[diagnostic(
        code(cargo_espflash::no_artifact),
        help(
            "If you're trying to run an example you need to specify it using the `--example` argument.\n\
              If you're in a Cargo workspace, specify the binary package with `--package`."
        )
    )]
    NoArtifact,

    #[error("'build-std' not configured")]
    #[diagnostic(
        code(cargo_espflash::no_build_std),
        help(
            "Cargo currently requires the unstable 'build-std' feature, ensure \
            that `.cargo/config{{.toml}}` has the appropriate options."
        ),
        url("https://doc.rust-lang.org/cargo/reference/unstable.html#build-std")
    )]
    NoBuildStd,

    #[error("No package could be located in the current workspace")]
    #[diagnostic(
        code(cargo_espflash::no_package),
        help(
            "Ensure that you are executing from a valid package, and that the specified package name \
              exists in the current workspace."
        )
    )]
    NoPackage,

    #[error("No `Cargo.toml` found in the current directory")]
    #[diagnostic(
        code(cargo_espflash::no_project),
        help("Ensure that you're running the command from within a Cargo project")
    )]
    NoProject,
}

/// TOML deserialization error
#[derive(Debug)]
pub struct TomlError {
    err: toml::de::Error,
    source: String,
}

impl TomlError {
    /// Create a new [`TomlError`] from the raw `toml`.
    pub fn new(err: toml::de::Error, source: String) -> Self {
        Self { err, source }
    }
}

impl Display for TomlError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "Failed to parse toml")
    }
}

impl Diagnostic for TomlError {
    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.source)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(once(LabeledSpan::new_with_span(
            Some(self.err.to_string()),
            self.err.span()?,
        ))))
    }
}

// NOTE: no `source` on purpose to prevent duplicating the message
impl std::error::Error for TomlError {}

/// Unsupported target error
#[derive(Debug, Diagnostic, Error)]
#[error("Target {target} is not supported by the {chip}")]
#[diagnostic(
    code(cargo_espflash::unsupported_target),
    help("The following targets are supported by the {}: {}", self.chip, self.supported_targets())
)]
pub struct UnsupportedTargetError {
    target: String,
    chip: Chip,
}

impl UnsupportedTargetError {
    /// Construct a new [`UnsupportedTargetError`].
    pub fn new(target: &str, chip: Chip) -> Self {
        Self {
            target: target.into(),
            chip,
        }
    }

    fn supported_targets(&self) -> String {
        self.chip.supported_build_targets().join(", ")
    }
}

/// No target error
#[derive(Debug, Error)]
#[error("No target specified in cargo configuration")]
pub struct NoTargetError {
    chip: Option<Chip>,
}

impl NoTargetError {
    /// Create a new [`NoTargetError`].
    pub fn new(chip: Option<Chip>) -> Self {
        Self { chip }
    }
}

impl Diagnostic for NoTargetError {
    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        Some(Box::new("cargo_espflash::no_target"))
    }

    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        Some(Box::new(match &self.chip {
            Some(chip) => format!(
                "Specify the target in `.cargo/config.toml`, the {} support the following targets: {}",
                chip,
                chip.supported_build_targets().join(", ")
            ),
            None => "Specify the target in `.cargo/config.toml`".into(),
        }))
    }
}
