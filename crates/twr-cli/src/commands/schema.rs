//! `schema` subcommand: prints expected CSV column layouts.

use twr_core::{
    ACTIVITY_OPTIONAL_COLUMNS, ACTIVITY_REQUIRED_COLUMNS, WAR_OPTIONAL_COLUMNS,
    WAR_REQUIRED_COLUMNS,
};

use crate::events::{self, Event};
use crate::EmitMode;

pub fn run(emit: EmitMode) -> anyhow::Result<()> {
    if emit == EmitMode::Json {
        events::emit(&Event::Schema {
            war_required: WAR_REQUIRED_COLUMNS.iter().map(|s| s.to_string()).collect(),
            war_optional: WAR_OPTIONAL_COLUMNS.iter().map(|s| s.to_string()).collect(),
            activity_required: ACTIVITY_REQUIRED_COLUMNS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            activity_optional: ACTIVITY_OPTIONAL_COLUMNS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            filename_convention: "<prefix>_<YYYY-MM-DDTHH-MM-SSZ>.csv".into(),
        });
    } else {
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
    }
    Ok(())
}
