//! `schema` subcommand: prints expected CSV column layouts.

use twr_core::{
    ACTIVITY_OPTIONAL_COLUMNS, ACTIVITY_REQUIRED_COLUMNS, WAR_OPTIONAL_COLUMNS,
    WAR_REQUIRED_COLUMNS,
};

pub fn run() -> anyhow::Result<()> {
    println!("== War CSV ==");
    println!("Required columns (case-sensitive):");
    for c in WAR_REQUIRED_COLUMNS {
        println!("  - {c}");
    }
    println!("Optional / preserved columns:");
    for c in WAR_OPTIONAL_COLUMNS {
        println!("  - {c}");
    }
    println!("Filename convention: <prefix>_<YYYY-MM-DDTHH-MM-SSZ>.csv");
    println!();
    println!("== Activity CSV ==");
    println!("Required columns:");
    for c in ACTIVITY_REQUIRED_COLUMNS {
        println!("  - {c}");
    }
    println!("Optional columns (preserved in `extras`):");
    for c in ACTIVITY_OPTIONAL_COLUMNS {
        println!("  - {c}");
    }
    Ok(())
}
