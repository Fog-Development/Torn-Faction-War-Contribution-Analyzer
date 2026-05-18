//! Core domain model for war participation analysis.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::config::Config;
use crate::warnings::Warning;

pub type MemberId = u64;
pub type MemberName = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct War {
    pub display_name: String,
    pub start_utc: DateTime<Utc>,
    pub source_filename: String,
    pub participants: Vec<WarParticipant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarParticipant {
    pub name: MemberName,
    pub id: Option<MemberId>,
    pub hits: u32,
    pub war_hits: u32,
    pub points: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberActivity {
    pub name: MemberName,
    pub days: Option<u32>,
    pub avg_e30: Option<f64>,
    pub extras: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarCategory {
    Excluded,
    Zero,
    Low,
    Ok,
}

impl WarCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            WarCategory::Excluded => "Excluded",
            WarCategory::Zero => "Zero",
            WarCategory::Low => "Low",
            WarCategory::Ok => "Ok",
        }
    }
}

impl std::fmt::Display for WarCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberWarResult {
    pub war_index: usize,
    pub category: WarCategory,
    pub points: u32,
    pub hits: u32,
    pub listed_in_csv: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberSummary {
    pub name: MemberName,
    pub days: Option<u32>,
    pub avg_e30: Option<f64>,
    pub wars: Vec<MemberWarResult>,
    pub present_count: u32,
    pub zero_count: u32,
    pub low_count: u32,
    pub poor_count: u32,
    pub avg_points: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub reference_time: DateTime<Utc>,
    pub config: Config,
    pub wars: Vec<War>,
    pub war_thresholds: Vec<f64>,
    pub members: Vec<MemberSummary>,
    pub auto_kick: Vec<MemberName>,
    pub repeat_offenders: Vec<MemberName>,
    pub any_bad_war: Vec<MemberName>,
    pub low_activity: Vec<MemberName>,
    pub combined_kick: Vec<MemberName>,
    pub warnings: Vec<Warning>,
}
