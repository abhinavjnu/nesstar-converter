use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SurveyMetadata {
    pub blocks: Vec<BlockDefinition>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlockDefinition {
    pub file_id: String,
    pub file_id_number: u32,
    pub name: String,
    pub row_count: u64,
    pub variables: Vec<VariableDefinition>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VariableDefinition {
    pub name: String,
    pub label: String,
    pub declared_type: DeclaredType,
    pub ddi_width: u32,
    pub decimals: u16,
    pub range: Option<NumericRange>,
    /// File IDs from the DDI `files` attribute, in declaration order.
    pub referenced_file_ids: Vec<String>,
}

/// The type declared by DDI's `varFormat` element.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum DeclaredType {
    Character,
    Numeric,
    Other(String),
}

impl DeclaredType {
    pub fn from_ddi(value: Option<&str>) -> Self {
        match value
            .unwrap_or("character")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "character" | "string" => Self::Character,
            "numeric" | "number" => Self::Numeric,
            other => Self::Other(other.to_owned()),
        }
    }
}

/// Numeric bounds declared by a DDI `range` element.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct NumericRange {
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum CellValue {
    Missing,
    Text(String),
}
