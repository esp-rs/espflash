use std::{
    cmp::Ordering,
    collections::HashMap,
    ffi::OsStr,
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
};

use clap::{Args, Parser};
use pyo3::{
    Bound,
    PyAny,
    prelude::{PyResult, Python},
    types::{PyAnyMethods as _, PyDict, PyList, PyModule, PyTuple},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// ----------------------------------------------------------------------------
// Command-line Interface

#[derive(Debug, Parser)]
enum Cli {
    /// Generate eFuse field definitions
    GenerateEfuseFields(GenerateEfuseFieldsArgs),
}

#[derive(Debug, Args)]
pub(crate) struct GenerateEfuseFieldsArgs {
    /// Local path to the `esptool` repository
    esptool_path: PathBuf,
}

const HEADER: &str = r#"
//! eFuse field definitions for the $CHIP
//!
//! This file was automatically generated, please do not edit it manually!
//! 
//! Generated: $DATE
//! Version:   $VERSION

#![allow(unused)]

use super::{EfuseBlock, EfuseField};

"#;

type EfuseFields = HashMap<String, EfuseYaml>;

#[derive(Debug, serde::Deserialize)]
struct EfuseYaml {
    #[serde(rename = "VER_NO")]
    version: String,
    #[serde(rename = "EFUSES")]
    fields: HashMap<String, EfuseAttrs>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct EfuseAttrs {
    #[serde(rename = "blk")]
    block: u32,
    word: u32,
    len: u32,
    start: u32,
    #[serde(rename = "desc")]
    description: String,
}

impl PartialOrd for EfuseAttrs {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EfuseAttrs {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.block.cmp(&other.block) {
            Ordering::Equal => {}
            ord => return ord,
        }

        match self.word.cmp(&other.word) {
            Ordering::Equal => {}
            ord => return ord,
        }

        self.start.cmp(&other.start)
    }
}

pub(crate) fn generate_efuse_fields(workspace: &Path, args: GenerateEfuseFieldsArgs) -> Result<()> {
    let efuse_yaml_path = args
        .esptool_path
        .join("espefuse")
        .join("efuse_defs")
        .canonicalize()?;

    let espflash_path = workspace.join("espflash").canonicalize()?;

    let mut efuse_fields = parse_efuse_fields(&efuse_yaml_path)?;
    process_efuse_definitions(&mut efuse_fields)?;
    generate_efuse_definitions(&espflash_path, &args.esptool_path, efuse_fields)?;

    Command::new("cargo")
        .args(["+nightly", "fmt"])
        .current_dir(workspace)
        .output()?;

    Ok(())
}

fn parse_efuse_fields(efuse_yaml_path: &Path) -> Result<EfuseFields> {
    // TODO: We can probably handle this better, e.g. by defining a `Chip` enum
    //       which can be iterated over, but for now this is good enough.
    const CHIPS: &[&str] = &[
        "esp32", "esp32c2", "esp32c3", "esp32c5", "esp32c6", "esp32h2", "esp32p4", "esp32s2",
        "esp32s3",
    ];

    let mut efuse_fields = EfuseFields::new();

    for result in fs::read_dir(efuse_yaml_path)? {
        let path = result?.path();
        if path.extension().is_none_or(|ext| ext != OsStr::new("yaml")) {
            continue;
        }

        let chip = path.file_stem().unwrap().to_string_lossy().to_string();
        if !CHIPS.contains(&chip.as_str()) {
            continue;
        }

        let efuse_yaml = fs::read_to_string(&path)?;
        let efuse_yaml: EfuseYaml = serde_yaml::from_str(&efuse_yaml)?;

        efuse_fields.insert(chip.to_string(), efuse_yaml);
    }

    Ok(efuse_fields)
}

fn process_efuse_definitions(efuse_fields: &mut EfuseFields) -> Result<()> {
    // This is all a special case for the MAC field, which is larger than a single
    // word (i.e. 32-bits) in size. To handle this, we just split it up into two
    // separate fields, and update the fields' attributes accordingly.
    for yaml in (*efuse_fields).values_mut() {
        let mac_attrs = yaml.fields.get("MAC").unwrap();

        let mut mac0_attrs = mac_attrs.clone();
        mac0_attrs.len = 32;

        let mut mac1_attrs = mac_attrs.clone();
        mac1_attrs.start = mac0_attrs.start + 32;
        mac1_attrs.word = mac1_attrs.start / 32;
        mac1_attrs.len = 16;

        yaml.fields.remove("MAC").unwrap();
        yaml.fields.insert("MAC0".into(), mac0_attrs);
        yaml.fields.insert("MAC1".into(), mac1_attrs);
    }

    // The ESP32-S2 seems to be missing a reserved byte at the end of BLOCK0
    // (Or, something else weird is going on).
    efuse_fields.entry("esp32s2".into()).and_modify(|yaml| {
        yaml.fields
            .entry("RESERVED_0_162".into())
            .and_modify(|field| field.len = 30);
    });

    Ok(())
}

fn generate_efuse_definitions(
    espflash_path: &Path,
    esptool_path: &Path,
    efuse_fields: EfuseFields,
) -> Result<()> {
    let targets_efuse_path = espflash_path
        .join("src")
        .join("target")
        .join("efuse")
        .canonicalize()?;

    for (chip, yaml) in efuse_fields {
        let f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(targets_efuse_path.join(format!("{chip}.rs")))?;

        let mut writer = BufWriter::new(f);

        write!(
            writer,
            "{}",
            HEADER
                .replace("$CHIP", &chip)
                .replace(
                    "$DATE",
                    &chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string()
                )
                .replace("$VERSION", &yaml.version)
                .trim_start()
        )?;

        println!("Processing {chip}");
        generate_efuse_blocks(&mut writer, esptool_path, &chip)?;
        generate_efuse_registers(&mut writer, esptool_path, &chip)?;
        generate_efuse_constants(&mut writer, &yaml.fields)?;
    }

    Ok(())
}

fn generate_efuse_blocks(writer: &mut dyn Write, esptool_path: &Path, chip: &str) -> Result<()> {
    write!(
        writer,
        r#"/// All eFuse blocks available on this device.
pub(crate) const BLOCKS: &[EfuseBlock] = &["#
    )?;

    python_definitions(esptool_path, chip, |_, mem_definition| {
        let blocks = mem_definition
            .getattr("EfuseDefineBlocks")?
            .call0()?
            .getattr("BLOCKS")?;
        let blocks = blocks.cast::<PyList>()?;

        let mut previous_index = None;
        for block in blocks {
            let block = block.cast::<PyTuple>()?;

            let index: u8 = block.get_item(2)?.extract()?;
            let length: u8 = block.get_item(7)?.extract()?;
            let read_address: u32 = block.get_item(3)?.extract()?;
            let write_address: u32 = block.get_item(4)?.extract()?;

            if let Some(previous_index) = previous_index {
                assert!(
                    (previous_index + 1) == index,
                    "Block indices should be sequential"
                );
            } else {
                assert!(index == 0, "Block indices should start at 0");
            }
            previous_index.replace(index);

            write!(
                writer,
                r#"
    EfuseBlock {{
        index: {index}u8,
        length: {length}u8,
        read_address: {read_address:#x}u32,
        write_address: {write_address:#x}u32,
    }},
"#,
            )?;
        }

        PyResult::Ok(())
    })
    .unwrap();

    writeln!(writer, r#"];"#)?;

    Ok(())
}

fn generate_efuse_registers(writer: &mut dyn Write, esptool_path: &Path, chip: &str) -> Result<()> {
    write!(
        writer,
        r#"
/// Defined eFuse registers and commands
pub(crate) mod defines {{
"#
    )?;

    python_definitions(esptool_path, chip, |inspect, mem_definition| {
        let registers = mem_definition.getattr("EfuseDefineRegisters")?.call0()?;

        let members = inspect.getattr("getmembers")?.call((&registers,), None)?;
        let members = PyDict::from_sequence(members.cast::<PyList>()?)?;
        let mut members: HashMap<String, Bound<'_, PyAny>> = members.extract()?;

        {
            write!(
                writer,
                "  use super::super::EfuseBlockErrors;
  pub(crate) const BLOCK_ERRORS: &[EfuseBlockErrors] = &["
            )?;

            let block_error_entry = |writer: &mut dyn Write,
                                     err_num_reg: u32,
                                     err_num_mask,
                                     err_num_offset,
                                     fail_bit_reg: u32,
                                     fail_bit_offset| {
                let map = |v: Option<u32>| {
                    v.map(|v| format!("Some({v:#x}u32)"))
                        .unwrap_or_else(|| "None".to_string())
                };
                writeln!(
                    writer,
                    "
    EfuseBlockErrors {{
        err_num_reg: {err_num_reg:#x}u32,
        err_num_mask: {},
        err_num_offset: {},
        fail_bit_reg: {fail_bit_reg:#x}u32,
        fail_bit_offset: {},
    }},",
                    map(err_num_mask),
                    map(err_num_offset),
                    map(fail_bit_offset),
                )
            };

            if let Some(block_errors) = members.remove("BLOCK_ERRORS") {
                for block_error in block_errors.cast::<PyList>()? {
                    let (err_reg, err_num_mask, err_num_offset, fail_bit_offset): (
                        u32,
                        Option<u32>,
                        Option<u32>,
                        Option<u32>,
                    ) = block_error.extract()?;
                    block_error_entry(
                        writer,
                        err_reg,
                        err_num_mask,
                        err_num_offset,
                        err_reg,
                        fail_bit_offset,
                    )?;
                }
            } else if members.contains_key("BLOCK_FAIL_BIT")
                && members.contains_key("BLOCK_NUM_ERRORS")
            {
                // ESP32-C3 has a design flaw where the fail bit is shifted by one block, so its
                // memory definition differs from the rest.
                let num_errors = members
                    .remove("BLOCK_NUM_ERRORS")
                    .unwrap()
                    .cast_into::<PyList>()?;
                let fail_bit = members
                    .remove("BLOCK_FAIL_BIT")
                    .unwrap()
                    .cast_into::<PyList>()?;

                for (num_errors, fail_bit) in num_errors.into_iter().zip(fail_bit.into_iter()) {
                    let (err_num_reg, err_num_mask, err_num_offset): (
                        u32,
                        Option<u32>,
                        Option<u32>,
                    ) = num_errors.extract()?;
                    let (fail_bit_reg, fail_bit_offset): (u32, Option<u32>) = fail_bit.extract()?;
                    block_error_entry(
                        writer,
                        err_num_reg,
                        err_num_mask,
                        err_num_offset,
                        fail_bit_reg,
                        fail_bit_offset,
                    )?;
                }
            }

            writeln!(writer, "  ];")?;
        }

        for (name, value) in members {
            if ["EFUSE_", "CODING_"]
                .iter()
                .any(|prefix| name.starts_with(prefix))
            {
                let Ok(value) = value.extract::<u32>() else {
                    continue;
                };

                writeln!(writer, "  pub(crate) const {name}: u32 = {value:#x};")?;
            }
        }

        PyResult::Ok(())
    })
    .unwrap();

    writeln!(writer, "}}")?;

    Ok(())
}

fn generate_efuse_constants(
    writer: &mut dyn Write,
    fields: &HashMap<String, EfuseAttrs>,
) -> Result<()> {
    let mut sorted = fields.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| (a.1).cmp(b.1));

    writeln!(writer)?;
    for (name, attrs) in sorted {
        let EfuseAttrs {
            block,
            word,
            len,
            start,
            description,
        } = attrs;

        let description = description.replace('[', "\\[").replace(']', "\\]");

        writeln!(writer, "/// {description}")?;
        writeln!(
            writer,
            "pub const {name}: EfuseField = EfuseField::new({block}, {word}, {start}, {len});"
        )?;
    }

    Ok(())
}

fn python_definitions<F>(esptool_path: &Path, chip: &str, f: F) -> PyResult<()>
where
    F: FnOnce(Bound<'_, PyModule>, Bound<'_, PyModule>) -> PyResult<()>,
{
    Python::attach(|py| {
        let sys = py.import("sys")?;
        let path = sys.getattr("path")?;
        path.call_method1("append", (esptool_path.as_os_str(),))?;

        let inspect = py.import("inspect")?;

        let mem_definition = py.import(format!("espefuse.efuse.{chip}.mem_definition"))?;

        f(inspect, mem_definition)
    })
}
