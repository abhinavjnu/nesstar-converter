//! Apache Parquet writer (enabled by the `parquet` feature).

#[cfg(feature = "parquet")]
pub use inner::ParquetOutput;

#[cfg(feature = "parquet")]
mod inner {
    use std::{fs::File, io::Write, path::Path, sync::Arc};

    use arrow_array::{Float64Array, RecordBatch as ArrowBatch, StringArray, array::ArrayRef};
    use arrow_schema::{DataType, Field, Schema};
    use parquet::{arrow::ArrowWriter, file::properties::WriterProperties};

    use crate::{
        NesstarError,
        decode::RecordBatch,
        model::{CellValue, DeclaredType, VariableDefinition},
    };

    pub struct ParquetOutput<W: Write + Send + 'static = File> {
        writer: ArrowWriter<W>,
        schema: Arc<Schema>,
        variables: Vec<VariableDefinition>,
    }

    fn field_for(var: &VariableDefinition) -> Field {
        let dtype = match var.declared_type {
            DeclaredType::Numeric | DeclaredType::Other(_) => DataType::Float64,
            DeclaredType::Character => DataType::Utf8,
        };
        Field::new(&var.name, dtype, true)
    }

    impl ParquetOutput<File> {
        pub fn create(path: &Path, variables: &[VariableDefinition]) -> Result<Self, NesstarError> {
            let file = File::create(path).map_err(|e| {
                NesstarError::Unsupported(format!("cannot create Parquet {}: {e}", path.display()))
            })?;
            Self::from_writer(file, variables)
        }
    }

    impl<W: Write + Send + 'static> ParquetOutput<W> {
        pub fn from_writer(
            writer: W,
            variables: &[VariableDefinition],
        ) -> Result<Self, NesstarError> {
            let fields: Vec<Field> = variables.iter().map(field_for).collect();
            let schema = Arc::new(Schema::new(fields));
            let props = WriterProperties::builder().build();
            let writer = ArrowWriter::try_new(writer, schema.clone(), Some(props))
                .map_err(|e| NesstarError::Unsupported(format!("Parquet writer: {e}")))?;

            Ok(Self {
                writer,
                schema,
                variables: variables.to_vec(),
            })
        }

        pub fn write_batch(&mut self, batch: &RecordBatch) -> Result<(), String> {
            let columns: Vec<ArrayRef> = self
                .variables
                .iter()
                .enumerate()
                .map(|(i, var)| -> ArrayRef {
                    let col = &batch.columns[i];
                    match var.declared_type {
                        DeclaredType::Numeric | DeclaredType::Other(_) => {
                            let vals: Vec<Option<f64>> = col
                                .values
                                .iter()
                                .map(|v| match v {
                                    CellValue::Missing => None,
                                    CellValue::Text(s) => s.trim().parse::<f64>().ok(),
                                })
                                .collect();
                            Arc::new(Float64Array::from(vals))
                        }
                        DeclaredType::Character => {
                            let vals: Vec<Option<&str>> = col
                                .values
                                .iter()
                                .map(|v| match v {
                                    CellValue::Missing => None,
                                    CellValue::Text(s) => Some(s.as_str()),
                                })
                                .collect();
                            Arc::new(StringArray::from(vals))
                        }
                    }
                })
                .collect();

            let arrow_batch =
                ArrowBatch::try_new(self.schema.clone(), columns).map_err(|e| e.to_string())?;
            self.writer.write(&arrow_batch).map_err(|e| e.to_string())
        }

        pub fn finish(self) -> Result<W, String> {
            self.writer.into_inner().map_err(|e| e.to_string())
        }
    }
}
