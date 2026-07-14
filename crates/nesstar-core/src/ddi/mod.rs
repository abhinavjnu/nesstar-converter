//! DDI XML parsing.

use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use quick_xml::{Reader, events::Event};

use crate::{
    NesstarError,
    model::{BlockDefinition, DeclaredType, NumericRange, SurveyMetadata, VariableDefinition},
};

/// Parse a DDI codebook into the shared survey model.
///
/// Both default-namespaced and non-namespaced DDI documents are accepted. XML
/// namespace prefixes are intentionally ignored because the Python reference
/// implementation matches elements by their DDI local name.
pub fn parse_ddi(path: impl AsRef<Path>) -> Result<SurveyMetadata, NesstarError> {
    let path = path.as_ref();
    let file = File::open(path)
        .map_err(|error| invalid_ddi(path, format!("cannot open file: {error}")))?;
    parse_ddi_reader(BufReader::new(file)).map_err(|error| match error {
        NesstarError::InvalidDdi(reason) => invalid_ddi(path, reason),
        other => other,
    })
}

/// Parse DDI data from a reader. This is primarily useful for callers that
/// already own the XML bytes and for focused parser tests.
pub fn parse_ddi_reader<R: Read>(reader: R) -> Result<SurveyMetadata, NesstarError> {
    let mut xml = Reader::from_reader(BufReader::new(reader));
    xml.config_mut().trim_text(true);

    let mut buffer = Vec::new();
    let mut blocks = Vec::new();
    let mut current_block: Option<PendingBlock> = None;
    let mut current_variable: Option<PendingVariable> = None;
    let mut text_target: Option<TextTarget> = None;

    loop {
        match xml.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) => {
                let name = local_name(element.name().as_ref()).to_vec();
                match name.as_slice() {
                    b"fileDscr" => {
                        if current_block.is_some() {
                            return Err(invalid("nested fileDscr elements are not supported"));
                        }
                        current_block = Some(PendingBlock::from_attributes(&element)?);
                    }
                    b"var" => {
                        if current_variable.is_some() {
                            return Err(invalid("nested var elements are not supported"));
                        }
                        current_variable = Some(PendingVariable::from_attributes(&element)?);
                    }
                    b"location" => set_location(&mut current_variable, &element)?,
                    b"varFormat" => set_format(&mut current_variable, &element)?,
                    b"range" => set_range(&mut current_variable, &element)?,
                    b"caseQnty" if current_block.is_some() => {
                        text_target = Some(TextTarget::CaseQuantity)
                    }
                    b"labl" if current_variable.is_some() => text_target = Some(TextTarget::Label),
                    _ => {}
                }
            }
            Ok(Event::Empty(element)) => {
                let name = local_name(element.name().as_ref()).to_vec();
                match name.as_slice() {
                    b"fileDscr" => blocks.push(PendingBlock::from_attributes(&element)?.finish()),
                    b"var" => {
                        let variable = PendingVariable::from_attributes(&element)?;
                        append_variable(&mut blocks, variable.finish())?;
                    }
                    b"location" => set_location(&mut current_variable, &element)?,
                    b"varFormat" => set_format(&mut current_variable, &element)?,
                    b"range" => set_range(&mut current_variable, &element)?,
                    _ => {}
                }
            }
            Ok(Event::Text(text)) => {
                let value = std::str::from_utf8(text.as_ref())
                    .map_err(|error| invalid(format!("text is not UTF-8: {error}")))?
                    .trim();
                match text_target {
                    Some(TextTarget::CaseQuantity) => {
                        let block = current_block
                            .as_mut()
                            .ok_or_else(|| invalid("caseQnty without fileDscr"))?;
                        block.row_count = parse_u64(value, "caseQnty")?;
                    }
                    Some(TextTarget::Label) => {
                        let variable = current_variable
                            .as_mut()
                            .ok_or_else(|| invalid("labl without var"))?;
                        variable.label.push_str(value);
                    }
                    None => {}
                }
            }
            Ok(Event::End(element)) => match local_name(element.name().as_ref()) {
                b"fileDscr" => {
                    let block = current_block
                        .take()
                        .ok_or_else(|| invalid("closing fileDscr without opening element"))?;
                    blocks.push(block.finish());
                }
                b"var" => {
                    let variable = current_variable
                        .take()
                        .ok_or_else(|| invalid("closing var without opening element"))?;
                    append_variable(&mut blocks, variable.finish())?;
                }
                b"caseQnty" | b"labl" => text_target = None,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(error) => return Err(invalid(format!("XML parse error: {error}"))),
            _ => {}
        }
        buffer.clear();
    }

    if current_block.is_some() || current_variable.is_some() {
        return Err(invalid("unexpected end of XML inside a DDI element"));
    }

    blocks.sort_by_key(|block| block.file_id_number);
    Ok(SurveyMetadata { blocks })
}

fn append_variable(
    blocks: &mut [BlockDefinition],
    variable: VariableDefinition,
) -> Result<(), NesstarError> {
    for file_id in &variable.referenced_file_ids {
        if let Some(block) = blocks.iter_mut().find(|block| block.file_id == *file_id) {
            block.variables.push(variable.clone());
        }
    }
    Ok(())
}

fn set_location(
    variable: &mut Option<PendingVariable>,
    element: &quick_xml::events::BytesStart<'_>,
) -> Result<(), NesstarError> {
    let variable = variable
        .as_mut()
        .ok_or_else(|| invalid("location outside var"))?;
    variable.ddi_width = parse_u32(
        &attribute(element, b"width")?.unwrap_or_default(),
        "location width",
    )?;
    Ok(())
}

fn set_format(
    variable: &mut Option<PendingVariable>,
    element: &quick_xml::events::BytesStart<'_>,
) -> Result<(), NesstarError> {
    let variable = variable
        .as_mut()
        .ok_or_else(|| invalid("varFormat outside var"))?;
    variable.declared_type = DeclaredType::from_ddi(attribute(element, b"type")?.as_deref());
    if let Some(decimals) = attribute(element, b"dcml")? {
        variable.decimals = parse_u16(&decimals, "varFormat dcml")?;
    }
    Ok(())
}

fn set_range(
    variable: &mut Option<PendingVariable>,
    element: &quick_xml::events::BytesStart<'_>,
) -> Result<(), NesstarError> {
    let variable = variable
        .as_mut()
        .ok_or_else(|| invalid("range outside var"))?;
    variable.range = Some(NumericRange {
        minimum: optional_f64(attribute(element, b"min")?, "range min")?,
        maximum: optional_f64(attribute(element, b"max")?, "range max")?,
    });
    Ok(())
}

fn attribute(
    element: &quick_xml::events::BytesStart<'_>,
    wanted: &[u8],
) -> Result<Option<String>, NesstarError> {
    for attribute in element.attributes().with_checks(false) {
        let attribute =
            attribute.map_err(|error| invalid(format!("invalid XML attribute: {error}")))?;
        if local_name(attribute.key.as_ref()) == wanted {
            let value = std::str::from_utf8(attribute.value.as_ref())
                .map_err(|error| invalid(format!("attribute value is not UTF-8: {error}")))?;
            return Ok(Some(value.to_owned()));
        }
    }
    Ok(None)
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn parse_u64(value: &str, context: &str) -> Result<u64, NesstarError> {
    value
        .parse()
        .map_err(|error| invalid(format!("invalid {context} `{value}`: {error}")))
}

fn parse_u32(value: &str, context: &str) -> Result<u32, NesstarError> {
    value
        .parse()
        .map_err(|error| invalid(format!("invalid {context} `{value}`: {error}")))
}

fn parse_u16(value: &str, context: &str) -> Result<u16, NesstarError> {
    value
        .parse()
        .map_err(|error| invalid(format!("invalid {context} `{value}`: {error}")))
}

fn optional_f64(value: Option<String>, context: &str) -> Result<Option<f64>, NesstarError> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .parse()
                .map_err(|error| invalid(format!("invalid {context} `{value}`: {error}")))
        })
        .transpose()
}

fn invalid(reason: impl Into<String>) -> NesstarError {
    NesstarError::InvalidDdi(reason.into())
}

fn invalid_ddi(path: &Path, reason: impl Into<String>) -> NesstarError {
    invalid(format!("{}: {}", path.display(), reason.into()))
}

#[derive(Default)]
struct PendingBlock {
    file_id: String,
    name: String,
    row_count: u64,
}

impl PendingBlock {
    fn from_attributes(element: &quick_xml::events::BytesStart<'_>) -> Result<Self, NesstarError> {
        let file_id = attribute(element, b"ID")?.unwrap_or_default();
        let uri = attribute(element, b"URI")?.unwrap_or_default();
        let name = uri
            .split("Name=")
            .last()
            .filter(|name| !name.is_empty())
            .unwrap_or(&file_id)
            .to_owned();
        Ok(Self {
            file_id,
            name,
            row_count: 0,
        })
    }

    fn finish(self) -> BlockDefinition {
        let numeric_id: String = self.file_id.chars().filter(char::is_ascii_digit).collect();
        BlockDefinition {
            file_id: self.file_id,
            file_id_number: numeric_id.parse().unwrap_or(0),
            name: self.name,
            row_count: self.row_count,
            variables: Vec::new(),
        }
    }
}

struct PendingVariable {
    name: String,
    label: String,
    declared_type: DeclaredType,
    ddi_width: u32,
    decimals: u16,
    range: Option<NumericRange>,
    referenced_file_ids: Vec<String>,
}

impl PendingVariable {
    fn from_attributes(element: &quick_xml::events::BytesStart<'_>) -> Result<Self, NesstarError> {
        Ok(Self {
            name: attribute(element, b"name")?.unwrap_or_default(),
            label: String::new(),
            declared_type: DeclaredType::Character,
            ddi_width: 0,
            decimals: 0,
            range: None,
            referenced_file_ids: attribute(element, b"files")?
                .unwrap_or_default()
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
        })
    }

    fn finish(self) -> VariableDefinition {
        VariableDefinition {
            name: self.name,
            label: self.label,
            declared_type: self.declared_type,
            ddi_width: self.ddi_width,
            decimals: self.decimals,
            range: self.range,
            referenced_file_ids: self.referenced_file_ids,
        }
    }
}

#[derive(Clone, Copy)]
enum TextTarget {
    CaseQuantity,
    Label,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/synthetic")
            .join(name)
    }

    #[test]
    fn parses_namespaced_metadata_scan_ddi() {
        let metadata = parse_ddi(fixture("metadata-scan.ddi.xml")).unwrap();
        let block = &metadata.blocks[0];
        assert_eq!(block.file_id, "F1");
        assert_eq!(block.file_id_number, 1);
        assert_eq!(block.name, "metadata-scan");
        assert_eq!(block.row_count, 4);
        assert_eq!(
            block
                .variables
                .iter()
                .map(|variable| variable.name.as_str())
                .collect::<Vec<_>>(),
            ["ASCII", "OFFSET", "FLOAT"]
        );
        assert_eq!(
            block.variables[1].range,
            Some(NumericRange {
                minimum: Some(-2.0),
                maximum: Some(300.0)
            })
        );
        assert_eq!(block.variables[2].declared_type, DeclaredType::Numeric);
    }

    #[test]
    fn parses_non_namespaced_resource_index_ddi() {
        let metadata = parse_ddi(fixture("resource-index.ddi.xml")).unwrap();
        let block = &metadata.blocks[0];
        assert_eq!(block.file_id, "F2");
        assert_eq!(block.variables.len(), 10);
        assert_eq!(block.variables[1].label, "UTF8");
        assert_eq!(block.variables[1].ddi_width, 8);
        assert_eq!(block.variables[1].referenced_file_ids, ["F2"]);
    }

    #[test]
    fn rejects_invalid_numeric_contextually() {
        let error = parse_ddi_reader("<codeBook><fileDscr ID=\"F1\"><dimensns><caseQnty>nope</caseQnty></dimensns></fileDscr></codeBook>".as_bytes()).unwrap_err();
        assert!(error.to_string().contains("caseQnty"));
    }
}
