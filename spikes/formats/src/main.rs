//! Deliberately small compatibility spike: data stays strings from source to writer.
//! It is not production code and must not be added to the production workspace.

use std::{error::Error, fs, path::Path};

type Rows = Vec<Vec<&'static str>>;

const COLUMNS: [&str; 6] = [
    "code_01",
    "response_text",
    "unicode_text",
    "very_long_variable_name_that_exceeds_stata_thirty_two_characters",
    "a-b",
    "a b",
];

const LABELS: [&str; 6] = [
    "Leading-zero code",
    "Text containing CSV and spreadsheet edge cases",
    "Unicode text",
    "This deliberately long label is more than eighty characters long so Stata label truncation has a concrete adversarial case",
    "Collision source: a-b",
    "Collision source: a b",
];

fn rows() -> Rows {
    // 19 rows crosses the deliberately tiny verification batch boundary (7 rows).
    let seeds = [
        ("001", "", "नमस्ते", "00001"),
        ("002", "comma, quote: \"yes\"", "café", "00002"),
        ("003", "tab\tseparated", "東京", "00003"),
        ("004", "line one\nline two", "😀", "00004"),
        ("005", "plain ASCII", "مرحبا", "00005"),
        ("000", "leading zero survives", "Ångström", "00006"),
        ("", "empty code is missing", "", ""),
    ];
    (0..19)
        .map(|index| {
            let (code, text, unicode, long_code) = seeds[index % seeds.len()];
            vec![code, text, unicode, long_code, "left", "right"]
        })
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args().nth(1).unwrap_or_else(|| "verify-output".into());
    let output = Path::new(&output);
    fs::create_dir_all(output)?;
    let rows = rows();

    #[cfg(feature = "csv")]
    write_csv(output, &rows)?;
    #[cfg(feature = "parquet")]
    write_parquet(output, &rows)?;
    #[cfg(feature = "excel")]
    write_excel(output, &rows)?;
    #[cfg(feature = "stata")]
    return Err("Stata is intentionally blocked: no candidate has passed an independent DTA 117 round trip".into());

    Ok(())
}

#[cfg(feature = "csv")]
fn write_csv(output: &Path, rows: &Rows) -> Result<(), Box<dyn Error>> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .terminator(csv::Terminator::Any(b'\n'))
        .from_path(output.join("adversarial.csv"))?;
    writer.write_record(COLUMNS)?;
    for batch in rows.chunks(7) {
        for row in batch { writer.write_record(row)?; }
    }
    writer.flush()?;
    Ok(())
}

#[cfg(feature = "parquet")]
fn write_parquet(output: &Path, rows: &Rows) -> Result<(), Box<dyn Error>> {
    use std::sync::Arc;
    use arrow_array::{ArrayRef, RecordBatch, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use parquet::arrow::ArrowWriter;
    use parquet::file::properties::WriterProperties;

    let schema = Arc::new(Schema::new(COLUMNS.iter().map(|name| Field::new(*name, DataType::Utf8, false)).collect::<Vec<_>>()));
    let properties = WriterProperties::builder()
        .set_compression(parquet::basic::Compression::SNAPPY)
        .build();
    let file = fs::File::create(output.join("adversarial.parquet"))?;
    let mut writer = ArrowWriter::try_new(file, Arc::clone(&schema), Some(properties))?;
    for batch_rows in rows.chunks(7) {
        let arrays: Vec<ArrayRef> = (0..COLUMNS.len())
            .map(|column| Arc::new(StringArray::from(batch_rows.iter().map(|row| row[column]).collect::<Vec<_>>())) as ArrayRef)
            .collect();
        writer.write(&RecordBatch::try_new(Arc::clone(&schema), arrays)?)?;
    }
    writer.close()?;
    Ok(())
}

#[cfg(feature = "excel")]
fn write_excel(output: &Path, rows: &Rows) -> Result<(), Box<dyn Error>> {
    use rust_xlsxwriter::Workbook;
    let mut workbook = Workbook::new();
    let data = workbook.add_worksheet();
    data.set_name("Data 1")?;
    for (column, label) in LABELS.iter().enumerate() { data.write_string(0, column as u16, *label)?; }
    for (column, name) in COLUMNS.iter().enumerate() { data.write_string(1, column as u16, *name)?; }
    for (row_index, row) in rows.iter().enumerate() {
        for (column, value) in row.iter().enumerate() { data.write_string((row_index + 2) as u32, column as u16, *value)?; }
    }
    let variables = workbook.add_worksheet();
    variables.set_name("Variables")?;
    for (column, heading) in ["Variable", "Label", "Type", "Width", "Decimals", "Encoding", "Min", "Max"].iter().enumerate() {
        variables.write_string(0, column as u16, *heading)?;
    }
    for (row, (name, label)) in COLUMNS.iter().zip(LABELS).enumerate() {
        variables.write_string((row + 1) as u32, 0, *name)?;
        variables.write_string((row + 1) as u32, 1, label)?;
        variables.write_string((row + 1) as u32, 2, "string")?;
    }
    workbook.save(output.join("adversarial.xlsx"))?;
    Ok(())
}
