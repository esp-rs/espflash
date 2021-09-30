use miette::{Diagnostic, LabeledSpan, SourceCode, SourceOffset};
use std::fmt::{Display, Formatter};
use std::iter::once;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    #[error("No executable artifact found")]
    #[diagnostic(
        code(cargo_espflash::no_artifact),
        help("If you're trying to run an example you need to specify it using the `--example` argument")
    )]
    NoArtifact,
    #[error("'build-std' not configured")]
    #[diagnostic(
        code(cargo_espflash::build_std),
        help(
            "cargo currently requires the unstable 'build-std' feature, ensure \
            that .cargo/config{{.toml}} has the appropriate options.\n  \
            \tSee: https://doc.rust-lang.org/cargo/reference/unstable.html#build-std"
        )
    )]
    NoBuildStd,
    #[error("Multiple build artifacts found")]
    #[diagnostic(
        code(cargo_espflash::multiple_artifacts),
        help("Please specify which artifact to flash using --bin")
    )]
    MultipleArtifacts,
    #[error("Specified partition table is not a csv file")]
    #[diagnostic(code(cargo_espflash::partition_table_path))]
    InvalidPartitionTablePath,
    #[error("Specified bootloader table is not a bin file")]
    #[diagnostic(code(cargo_espflash::bootloader_path))]
    InvalidBootloaderPath,
    #[error("No Cargo.toml found in the current directory")]
    #[diagnostic(
        code(cargo_espflash::no_project),
        help("Ensure that you're running the command from within a cargo project")
    )]
    NoProject,
}

#[derive(Debug)]
pub struct TomlError {
    err: MaybeTomlError,
    source: String,
}

#[derive(Debug)]
pub enum MaybeTomlError {
    Toml(toml::de::Error),
    Other(std::io::Error),
}

impl From<cargo_toml::Error> for MaybeTomlError {
    fn from(e: cargo_toml::Error) -> Self {
        match e {
            cargo_toml::Error::Parse(e) => MaybeTomlError::Toml(e),
            cargo_toml::Error::Io(e) => MaybeTomlError::Other(e),
        }
    }
}

impl TomlError {
    pub fn new(err: impl Into<MaybeTomlError>, source: String) -> Self {
        TomlError {
            err: err.into(),
            source,
        }
    }
}

impl Display for TomlError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse toml")
    }
}

// no `source` on purpose to prevent duplicating the message
impl std::error::Error for TomlError {}

impl Diagnostic for TomlError {
    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.source)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        match &self.err {
            MaybeTomlError::Toml(err) => {
                let (line, col) = err.line_col()?;
                let offset = SourceOffset::from_location(&self.source, line + 1, col + 1);
                Some(Box::new(once(LabeledSpan::new(
                    Some(err.to_string()),
                    offset.offset(),
                    0,
                ))))
            }
            _ => None,
        }
    }
}
