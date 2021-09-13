use miette::Diagnostic;
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
}
