use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    ffi::OsStr,
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
};

use clap::{Args, Parser};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// ----------------------------------------------------------------------------
// Command-line Interface

#[derive(Debug, Parser)]
enum Cli {
    /// Generate eFuse field definitions
    GenerateEfuseFields(GenerateEfuseFieldsArgs),
}

#[derive(Debug, Args)]
struct GenerateEfuseFieldsArgs {
    /// Local path to the `esptool` repository
    esptool_path: PathBuf,
}

// ----------------------------------------------------------------------------
// Application

fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_module("xtask", log::LevelFilter::Info)
        .init();

    // The directory containing the cargo manifest for the 'xtask' package is a
    // subdirectory within the cargo workspace:
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = workspace.parent().unwrap().canonicalize()?;

    match Cli::parse() {
        Cli::GenerateEfuseFields(args) => generate_efuse_fields(&workspace, args),
    }
}

// ----------------------------------------------------------------------------
// Generate eFuse Fields

const HEADER: &str = r#"
//! This file was automatically generated, please do not edit it manually!
//! 
//! Generated: $DATE
//! Version:   $VERSION

#![allow(unused)]

use super::EfuseField;

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

fn generate_efuse_fields(workspace: &Path, args: GenerateEfuseFieldsArgs) -> Result<()> {
    let efuse_yaml_path = args
        .esptool_path
        .join("espefuse")
        .join("efuse_defs")
        .canonicalize()?;

    let espflash_path = workspace.join("espflash").canonicalize()?;

    let mut efuse_fields = parse_efuse_fields(&efuse_yaml_path)?;
    process_efuse_definitions(&mut efuse_fields)?;
    generate_efuse_definitions(&espflash_path, efuse_fields)?;

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
        mac0_attrs.start = 0;
        mac0_attrs.len = 32;

        let mut mac1_attrs = mac_attrs.clone();
        mac1_attrs.word += 1;
        mac1_attrs.start = 32;
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

fn generate_efuse_definitions(espflash_path: &Path, efuse_fields: EfuseFields) -> Result<()> {
    let targets_efuse_path = espflash_path
        .join("src")
        .join("targets")
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
                .replace(
                    "$DATE",
                    &chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string()
                )
                .replace("$VERSION", &yaml.version)
                .trim_start()
        )?;

        generate_efuse_block_sizes(&mut writer, &yaml.fields)?;
        generate_efuse_constants(&mut writer, &yaml.fields)?;
    }

    Ok(())
}

fn generate_efuse_block_sizes(
    writer: &mut dyn Write,
    fields: &HashMap<String, EfuseAttrs>,
) -> Result<()> {
    let mut field_attrs = fields.values().collect::<Vec<_>>();
    field_attrs.sort();

    let block_sizes = field_attrs
        .chunk_by(|a, b| a.block == b.block)
        .enumerate()
        .map(|(block, attrs)| {
            let last = attrs.last().unwrap();
            let size_bits = last.start + last.len;
            assert!(size_bits % 8 == 0);

            (block, size_bits / 8)
        })
        .collect::<BTreeMap<_, _>>();

    writeln!(writer, "/// Total size in bytes of each block")?;
    writeln!(
        writer,
        "pub(crate) const BLOCK_SIZES: &[u32] = &[{}];\n",
        block_sizes
            .values()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )?;

    Ok(())
}

fn generate_efuse_constants(
    writer: &mut dyn Write,
    fields: &HashMap<String, EfuseAttrs>,
) -> Result<()> {
    let mut sorted = fields.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| (a.1).cmp(b.1));

    for (name, attrs) in sorted {
        let EfuseAttrs {
            block,
            word,
            len,
            start,
            description,
        } = attrs;

        writeln!(writer, "/// {description}")?;
        writeln!(
            writer,
            "pub(crate) const {}: EfuseField = EfuseField::new({}, {}, {}, {});",
            name, block, word, start, len
        )?;
    }

    Ok(())
}
