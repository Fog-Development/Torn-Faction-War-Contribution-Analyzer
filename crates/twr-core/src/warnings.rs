//! Warning collection used throughout parsing and analysis.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarningKind {
    MalformedRow,
    DuplicateId,
    MissingActivityRecord,
    InsufficientTenure,
    AmbiguousMemberName,
    FilenameDateMissing,
    MissingColumn,
    MissingAttackerId,
    ParseError,
    InconsistentIdName,
}

impl std::fmt::Display for WarningKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            WarningKind::MalformedRow => "MalformedRow",
            WarningKind::DuplicateId => "DuplicateId",
            WarningKind::MissingActivityRecord => "MissingActivityRecord",
            WarningKind::InsufficientTenure => "InsufficientTenure",
            WarningKind::AmbiguousMemberName => "AmbiguousMemberName",
            WarningKind::FilenameDateMissing => "FilenameDateMissing",
            WarningKind::MissingColumn => "MissingColumn",
            WarningKind::MissingAttackerId => "MissingAttackerId",
            WarningKind::ParseError => "ParseError",
            WarningKind::InconsistentIdName => "InconsistentIdName",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    pub kind: WarningKind,
    pub source: String,
    pub detail: String,
    pub row_or_member: Option<String>,
}

impl Warning {
    pub fn new(
        kind: WarningKind,
        source: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            source: source.into(),
            detail: detail.into(),
            row_or_member: None,
        }
    }

    pub fn with_context(mut self, ctx: impl Into<String>) -> Self {
        self.row_or_member = Some(ctx.into());
        self
    }
}

/// Thread-safe accumulator for warnings emitted during parsing/analysis.
#[derive(Debug, Clone, Default)]
pub struct WarningCollector {
    inner: Arc<Mutex<Vec<Warning>>>,
}

impl WarningCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, warning: Warning) {
        tracing::warn!(
            kind = %warning.kind,
            source = %warning.source,
            detail = %warning.detail,
            "warning emitted"
        );
        self.inner.lock().expect("warnings lock poisoned").push(warning);
    }

    pub fn extend(&self, others: impl IntoIterator<Item = Warning>) {
        for w in others {
            self.push(w);
        }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("warnings lock poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn snapshot(&self) -> Vec<Warning> {
        self.inner.lock().expect("warnings lock poisoned").clone()
    }

    pub fn take(&self) -> Vec<Warning> {
        let mut guard = self.inner.lock().expect("warnings lock poisoned");
        std::mem::take(&mut *guard)
    }
}
