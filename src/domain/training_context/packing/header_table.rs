use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::Value;

use crate::domain::training_context::model::PlannedWorkoutBlockContext;

/// Header-mapped table: optional `def_*` defaults, then `h` column keys, then `r` rows.
pub(crate) struct HeaderTable {
    defaults: Vec<(String, Value)>,
    h: Vec<&'static str>,
    r: Vec<Vec<Value>>,
}

impl Serialize for HeaderTable {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        for (key, value) in &self.defaults {
            map.serialize_entry(key, value)?;
        }
        map.serialize_entry("h", &self.h)?;
        map.serialize_entry("r", &self.r)?;
        map.end()
    }
}

pub(crate) struct TableBuilder {
    defaults: Vec<(String, Value)>,
    columns: Vec<ColumnDef>,
    rows: Vec<Vec<CellValue>>,
}

struct ColumnDef {
    key: &'static str,
    optional: bool,
}

pub(crate) enum CellValue {
    Null,
    Value(Value),
}

impl TableBuilder {
    pub(crate) fn new(columns: &[(&'static str, bool)]) -> Self {
        Self {
            defaults: Vec::new(),
            columns: columns
                .iter()
                .map(|(key, optional)| ColumnDef {
                    key,
                    optional: *optional,
                })
                .collect(),
            rows: Vec::new(),
        }
    }

    pub(crate) fn def_str(mut self, key: &str, value: &str) -> Self {
        self.defaults
            .push((key.to_string(), Value::String(value.to_string())));
        self
    }

    pub(crate) fn push_row(mut self, cells: Vec<CellValue>) -> Self {
        debug_assert_eq!(cells.len(), self.columns.len());
        self.rows.push(cells);
        self
    }

    pub(crate) fn build(self) -> Option<HeaderTable> {
        if self.rows.is_empty() {
            return None;
        }

        let active: Vec<bool> = self
            .columns
            .iter()
            .enumerate()
            .map(|(idx, col)| {
                if !col.optional {
                    return true;
                }
                self.rows
                    .iter()
                    .any(|row| matches!(row[idx], CellValue::Value(_)))
            })
            .collect();

        let h: Vec<&'static str> = self
            .columns
            .iter()
            .zip(active.iter())
            .filter_map(|(col, on)| on.then_some(col.key))
            .collect();

        let r = self
            .rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .zip(active.iter())
                    .filter_map(|(cell, on)| {
                        on.then_some(match cell {
                            CellValue::Null => Value::Null,
                            CellValue::Value(value) => value,
                        })
                    })
                    .collect()
            })
            .collect();

        Some(HeaderTable {
            defaults: self.defaults,
            h,
            r,
        })
    }
}

pub(crate) fn cell_str(value: &str) -> CellValue {
    CellValue::Value(Value::String(value.to_string()))
}

pub(crate) fn cell_opt_str(value: Option<&str>) -> CellValue {
    match value.filter(|value| !value.is_empty()) {
        Some(value) => cell_str(value),
        None => CellValue::Null,
    }
}

pub(crate) fn cell_i32(value: i32) -> CellValue {
    CellValue::Value(Value::from(value))
}

pub(crate) fn cell_opt_i32(value: Option<i32>) -> CellValue {
    match value {
        Some(value) => cell_i32(value),
        None => CellValue::Null,
    }
}

pub(crate) fn cell_f64(value: f64) -> CellValue {
    CellValue::Value(
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
    )
}

pub(crate) fn cell_opt_f64(value: Option<f64>) -> CellValue {
    match value {
        Some(value) => cell_f64(value),
        None => CellValue::Null,
    }
}

pub(crate) fn cell_i64(value: i64) -> CellValue {
    CellValue::Value(Value::from(value))
}

pub(crate) fn cell_bool(value: bool) -> CellValue {
    CellValue::Value(Value::Bool(value))
}

pub(crate) fn cell_opt_u8(value: Option<u8>) -> CellValue {
    match value {
        Some(value) => CellValue::Value(Value::from(value)),
        None => CellValue::Null,
    }
}

pub(crate) fn cell_opt_u16(value: Option<u16>) -> CellValue {
    match value {
        Some(value) => CellValue::Value(Value::from(value)),
        None => CellValue::Null,
    }
}

pub(crate) fn cell_opt_json(value: Option<HeaderTable>) -> CellValue {
    cell_opt_value(value.and_then(|table| serde_json::to_value(table).ok()))
}

pub(crate) fn cell_opt_value(value: Option<Value>) -> CellValue {
    match value {
        Some(value) => CellValue::Value(value),
        None => CellValue::Null,
    }
}

pub(crate) fn cell_opt_segments(segments: &[[i32; 3]]) -> CellValue {
    if segments.is_empty() {
        CellValue::Null
    } else {
        CellValue::Value(Value::Array(
            segments
                .iter()
                .map(|segment| Value::Array(segment.iter().map(|v| Value::from(*v)).collect()))
                .collect(),
        ))
    }
}

pub(crate) fn interval_blocks_table(blocks: &[PlannedWorkoutBlockContext]) -> Option<HeaderTable> {
    if blocks.is_empty() {
        return None;
    }
    let mut builder = TableBuilder::new(&[
        ("dur", false),
        ("minp", true),
        ("maxp", true),
        ("minw", true),
        ("maxw", true),
    ]);
    for block in blocks {
        builder = builder.push_row(vec![
            cell_i32(block.duration_seconds),
            cell_opt_f64(block.min_percent_ftp),
            cell_opt_f64(block.max_percent_ftp),
            cell_opt_i32(block.min_target_watts),
            cell_opt_i32(block.max_target_watts),
        ]);
    }
    builder.build()
}

pub(crate) fn uniform_string(values: impl Iterator<Item = impl AsRef<str>>) -> Option<String> {
    let mut iter = values.map(|value| value.as_ref().to_string());
    let first = iter.next()?;
    if iter.all(|value| value == first) {
        Some(first)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_table_serializes_defaults_columns_and_rows() {
        let table = TableBuilder::new(&[("d", false), ("tss", false)])
            .push_row(vec![cell_str("2026-01-01"), cell_i32(80)])
            .build()
            .expect("table");
        let json = serde_json::to_value(&table).expect("json");
        assert_eq!(json["h"], serde_json::json!(["d", "tss"]));
        assert_eq!(json["r"], serde_json::json!([["2026-01-01", 80]]));
    }

    #[test]
    fn optional_columns_omitted_when_all_null() {
        let table = TableBuilder::new(&[("d", false), ("n", true)])
            .push_row(vec![cell_str("2026-01-01"), CellValue::Null])
            .build()
            .expect("table");
        let json = serde_json::to_value(&table).expect("json");
        assert_eq!(json["h"], serde_json::json!(["d"]));
        assert_eq!(json["r"], serde_json::json!([["2026-01-01"]]));
    }

    #[test]
    fn empty_builder_returns_none() {
        assert!(TableBuilder::new(&[("d", false)]).build().is_none());
    }
}
