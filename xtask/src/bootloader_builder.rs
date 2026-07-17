use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    ffi::OsStr,
    fs,
    io::{Error, ErrorKind},
    path::{Path, PathBuf},
    process::Command,
};

use clap::Args;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::Result;

#[derive(Debug, Args)]
pub(crate) struct BuildBootloadersArgs {
    /// Managed ESP-IDF checkout used to build the bootloaders
    #[arg(long, default_value = "target/esp-idf-bootloader")]
    esp_idf_path: PathBuf,

    /// Do not fetch ESP-IDF updates before checking out manifest revisions
    #[arg(long)]
    no_fetch: bool,

    /// Run ESP-IDF's install script for each target before building
    #[arg(long)]
    install_tools: bool,

    /// Only build the named bootloader entry. Can be passed multiple times
    #[arg(long)]
    only: Vec<String>,

    /// Keep the generated temporary ESP-IDF project/build directories
    #[arg(long)]
    keep_build_dirs: bool,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    defaults: Defaults,
    bootloaders: Vec<Bootloader>,
}

#[derive(Debug, Deserialize)]
struct Defaults {
    idf_ref: String,
}

#[derive(Debug, Deserialize)]
struct Bootloader {
    name: String,
    target: String,
    output: PathBuf,
    idf_ref: Option<String>,
    #[serde(default)]
    preview: bool,
    #[serde(default)]
    configs: Vec<String>,
}

pub(crate) fn build_bootloaders(workspace: &Path, args: BuildBootloadersArgs) -> Result<()> {
    let resources_dir = workspace.join("espflash").join("resources");
    let bootloaders_dir = resources_dir.join("bootloaders");
    let manifest_path = bootloaders_dir.join("manifest.yaml");
    let manifest = fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_yaml::from_str(&manifest)?;

    let selected = selected_bootloaders(&manifest, &args.only)?;
    ensure_esp_idf_checkout(&args.esp_idf_path, args.no_fetch)?;

    let idf_tools_path = env::var_os("IDF_TOOLS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target").join("esp-idf-tools"));

    let build_root = workspace.join("target").join("bootloader-build");
    if build_root.exists() && !args.keep_build_dirs {
        fs::remove_dir_all(&build_root)?;
    }
    fs::create_dir_all(&build_root)?;

    let mut current_ref = None;
    let mut installed = BTreeSet::new();

    for bootloader in selected {
        let idf_ref = bootloader
            .idf_ref
            .as_deref()
            .unwrap_or(&manifest.defaults.idf_ref);

        if current_ref.as_deref() != Some(idf_ref) {
            checkout_esp_idf_ref(&args.esp_idf_path, idf_ref)?;
            current_ref = Some(idf_ref.to_string());
        }

        if args.install_tools && installed.insert((idf_ref.to_string(), bootloader.target.clone()))
        {
            install_esp_idf_tools(&args.esp_idf_path, &idf_tools_path, &bootloader.target)?;
        }

        let generated =
            build_bootloader(&args.esp_idf_path, &idf_tools_path, &build_root, bootloader)?;
        let actual_sha = sha256_hex(&generated);

        let output = bootloaders_dir.join(&bootloader.output);
        fs::write(&output, generated)?;

        println!("{}: {actual_sha}", bootloader.name);
    }

    Ok(())
}

fn selected_bootloaders<'a>(
    manifest: &'a Manifest,
    only: &[String],
) -> Result<Vec<&'a Bootloader>> {
    if only.is_empty() {
        return Ok(manifest.bootloaders.iter().collect());
    }

    let by_name: BTreeMap<_, _> = manifest
        .bootloaders
        .iter()
        .map(|bootloader| (bootloader.name.as_str(), bootloader))
        .collect();

    only.iter()
        .map(|name| {
            by_name.get(name.as_str()).copied().ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidInput,
                    format!("unknown bootloader {name:?}; see espflash/resources/bootloaders/manifest.yaml"),
                )
                .into()
            })
        })
        .collect()
}

fn ensure_esp_idf_checkout(esp_idf_path: &Path, no_fetch: bool) -> Result<()> {
    if esp_idf_path.exists() {
        if !no_fetch {
            run(Command::new("git")
                .arg("-C")
                .arg(esp_idf_path)
                .arg("fetch")
                .arg("--tags"))?;
            run(Command::new("git")
                .arg("-C")
                .arg(esp_idf_path)
                .arg("fetch")
                .arg("origin"))?;
        }
        return Ok(());
    }

    if no_fetch {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!(
                "ESP-IDF checkout {} does not exist and --no-fetch was passed",
                esp_idf_path.display()
            ),
        )
        .into());
    }

    if let Some(parent) = esp_idf_path.parent() {
        fs::create_dir_all(parent)?;
    }

    run(Command::new("git")
        .arg("clone")
        .arg("https://github.com/espressif/esp-idf.git")
        .arg(esp_idf_path))?;

    Ok(())
}

fn checkout_esp_idf_ref(esp_idf_path: &Path, idf_ref: &str) -> Result<()> {
    let version_txt = esp_idf_path.join("version.txt");
    if version_txt.exists() {
        fs::remove_file(version_txt)?;
    }

    run(Command::new("git")
        .arg("-C")
        .arg(esp_idf_path)
        .arg("checkout")
        .arg("--force")
        .arg(idf_ref))?;
    run(Command::new("git")
        .arg("-C")
        .arg(esp_idf_path)
        .arg("submodule")
        .arg("update")
        .arg("--init")
        .arg("--recursive"))?;
    Ok(())
}

#[cfg(unix)]
fn install_esp_idf_tools(esp_idf_path: &Path, idf_tools_path: &Path, target: &str) -> Result<()> {
    run(Command::new("./install.sh")
        .arg(target)
        .current_dir(esp_idf_path)
        .env("IDF_TOOLS_PATH", idf_tools_path))
}

#[cfg(windows)]
fn install_esp_idf_tools(esp_idf_path: &Path, idf_tools_path: &Path, target: &str) -> Result<()> {
    run(Command::new("cmd")
        .args(["/C", "call", "install.bat"])
        .arg(target)
        .current_dir(esp_idf_path)
        .env("IDF_TOOLS_PATH", idf_tools_path))
}

fn build_bootloader(
    esp_idf_path: &Path,
    idf_tools_path: &Path,
    build_root: &Path,
    bootloader: &Bootloader,
) -> Result<Vec<u8>> {
    let project_dir = build_root.join(&bootloader.name);
    if project_dir.exists() {
        fs::remove_dir_all(&project_dir)?;
    }

    let main_dir = project_dir.join("main");
    fs::create_dir_all(&main_dir)?;
    fs::write(
        project_dir.join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.16)\ninclude($ENV{IDF_PATH}/tools/cmake/project.cmake)\nproject(espflash_bootloader)\n",
    )?;
    fs::write(
        main_dir.join("CMakeLists.txt"),
        "idf_component_register(SRCS \"main.c\" INCLUDE_DIRS \".\")\n",
    )?;
    fs::write(main_dir.join("main.c"), "void app_main(void) {}\n")?;

    let sdkconfig_defaults = project_dir.join("sdkconfig.defaults");
    fs::write(
        &sdkconfig_defaults,
        format!(
            "# Generated by xtask build-bootloaders\n{}",
            bootloader.configs.join("\n")
        ),
    )?;

    let build_dir = project_dir.join("build");
    run_idf_py(
        esp_idf_path,
        idf_tools_path,
        &project_dir,
        &build_dir,
        &sdkconfig_defaults,
        bootloader,
    )?;

    fs::read(build_dir.join("bootloader").join("bootloader.bin")).map_err(Into::into)
}

fn run(command: &mut Command) -> Result<()> {
    println!("+ {command:?}");
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::other(format!("command failed with status {status}: {command:?}")).into())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(unix)]
fn run_idf_py(
    esp_idf_path: &Path,
    idf_tools_path: &Path,
    project_dir: &Path,
    build_dir: &Path,
    sdkconfig_defaults: &Path,
    bootloader: &Bootloader,
) -> Result<()> {
    let preview_arg = if bootloader.preview { " --preview" } else { "" };
    let script = format!(
        "set -euo pipefail\nsource {}/export.sh >/dev/null\nidf.py{} -C {} -B {} -D SDKCONFIG_DEFAULTS={} set-target {}\nidf.py{} -C {} -B {} bootloader\n",
        shell_quote(esp_idf_path),
        preview_arg,
        shell_quote(project_dir),
        shell_quote(build_dir),
        shell_quote(sdkconfig_defaults),
        shell_quote(OsStr::new(&bootloader.target)),
        preview_arg,
        shell_quote(project_dir),
        shell_quote(build_dir),
    );

    run(Command::new("bash")
        .arg("-lc")
        .arg(script)
        .env("IDF_TOOLS_PATH", idf_tools_path))
}

#[cfg(windows)]
fn run_idf_py(
    esp_idf_path: &Path,
    idf_tools_path: &Path,
    project_dir: &Path,
    build_dir: &Path,
    sdkconfig_defaults: &Path,
    bootloader: &Bootloader,
) -> Result<()> {
    let preview_arg = if bootloader.preview { " --preview" } else { "" };
    let script = format!(
        "call {} >nul && idf.py{} -C {} -B {} -D SDKCONFIG_DEFAULTS={} set-target {} && idf.py{} -C {} -B {} bootloader",
        cmd_quote(&esp_idf_path.join("export.bat")),
        preview_arg,
        cmd_quote(project_dir),
        cmd_quote(build_dir),
        cmd_quote(sdkconfig_defaults),
        cmd_quote(OsStr::new(&bootloader.target)),
        preview_arg,
        cmd_quote(project_dir),
        cmd_quote(build_dir),
    );

    run(Command::new("cmd")
        .arg("/C")
        .arg(script)
        .env("IDF_TOOLS_PATH", idf_tools_path))
}

#[cfg(unix)]
fn shell_quote(value: impl AsRef<OsStr>) -> String {
    let value = value.as_ref().to_string_lossy();
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(windows)]
fn cmd_quote(value: impl AsRef<OsStr>) -> String {
    let value = value.as_ref().to_string_lossy();
    format!("\"{}\"", value.replace('"', "\\\""))
}
