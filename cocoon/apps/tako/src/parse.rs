use polars::io::{avro, json};
use polars::prelude::*;

/// An input file format accepted by the ingest API.
///
/// This enum is the single source of truth for which file types `/upload`
/// can decode. To support a new format, add a variant, map its extension in
/// [`FileType::from_file_name`], give it a stable metric label in
/// [`FileType::as_metric_label`], and handle it in [`parse_data`] — the
/// compiler's exhaustiveness check then forces every site to be updated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Csv,
    Parquet,
    /// JSON array or newline-delimited JSON (NDJSON); auto-detected at parse time.
    Json,
    Avro,
    Vortex,
    /// Any unrecognised extension. Kept as an explicit variant so metric
    /// cardinality stays bounded (everything unknown collapses to `other`).
    Other,
}

impl FileType {
    /// Classify a file by its (case-insensitive) extension.
    pub fn from_file_name(file_name: &str) -> Self {
        let extension = file_name.rsplit('.').next().unwrap_or("").to_lowercase();

        match extension.as_str() {
            "csv" => FileType::Csv,
            "parquet" => FileType::Parquet,
            // NDJSON shares JSON's reader, which auto-detects the framing.
            "json" | "ndjson" | "jsonl" => FileType::Json,
            "avro" => FileType::Avro,
            "vortex" | "vx" => FileType::Vortex,
            _ => FileType::Other,
        }
    }

    /// Stable, low-cardinality label for metrics and structured logging.
    ///
    /// Returning a bounded set of `&'static str` prevents an attacker (or a
    /// fat-fingered client) from blowing up metric cardinality with arbitrary
    /// extensions.
    ///
    /// Not yet wired into the upload metrics path — kept (and unit-tested) as the
    /// documented extension point for per-format metric labels.
    #[allow(dead_code)]
    pub fn as_metric_label(self) -> &'static str {
        match self {
            FileType::Csv => "csv",
            FileType::Parquet => "parquet",
            FileType::Json => "json",
            FileType::Avro => "avro",
            FileType::Vortex => "vortex",
            FileType::Other => "other",
        }
    }
}

/**
Parse data from a byte array into a DataFrame.

Supports CSV, Parquet, JSON/NDJSON, Avro, and Vortex. Parsing is eager rather
than lazy: the whole payload is already in memory (capped at the upload limit),
so there is no streaming benefit to deferring the work.
*/
pub fn parse_data(data: &[u8], file_name: &str) -> Result<DataFrame, PolarsError> {
    match FileType::from_file_name(file_name) {
        FileType::Csv => CsvReader::new(std::io::Cursor::new(data)).finish(),
        FileType::Parquet => ParquetReader::new(std::io::Cursor::new(data)).finish(),
        FileType::Json => parse_json(data),
        FileType::Avro => avro::AvroReader::new(std::io::Cursor::new(data)).finish(),
        FileType::Vortex => parse_vortex(data),
        FileType::Other => Err(PolarsError::ComputeError("Unsupported file type".into())),
    }
}

/// Decode a JSON payload, auto-detecting a top-level array vs NDJSON.
///
/// Polars' `JsonReader` defaults to the JsonLines (NDJSON) framing, so we must
/// switch to `Json` when the first non-whitespace byte is `[`.
fn parse_json(data: &[u8]) -> Result<DataFrame, PolarsError> {
    let first_non_ws = data.iter().position(|b| !b.is_ascii_whitespace());
    let format = if first_non_ws.map(|i| data[i] == b'[').unwrap_or(false) {
        JsonFormat::Json
    } else {
        JsonFormat::JsonLines
    };
    json::JsonReader::new(std::io::Cursor::new(data))
        .with_json_format(format)
        .finish()
}

/// Decode a Vortex file into a Polars `DataFrame`.
///
/// Vortex lives in the `arrow-rs` ecosystem while Polars uses `polars-arrow`,
/// so the two cannot share `RecordBatch`es directly. We bridge them through the
/// Arrow IPC stream format, which both implementations read and write: Vortex →
/// arrow-rs `RecordBatch` → IPC bytes → Polars `IpcStreamReader`.
///
/// Vortex's reader is async-first; we drive it on a `CurrentThreadRuntime`
/// (a `BlockingRuntime`) so this function stays synchronous like its siblings.
// `into_arrow_preferred` is deprecated in favour of `execute_arrow(None, ctx)`,
// which requires threading an ArrayContext through. The simpler call is fine for
// a one-shot, fully-materialised decode; revisit if Vortex removes it.
#[allow(deprecated)]
fn parse_vortex(data: &[u8]) -> Result<DataFrame, PolarsError> {
    use arrow::array::{Array, RecordBatch, StructArray};
    use vortex::VortexSessionDefault;
    use vortex::array::arrow::IntoArrowArray;
    use vortex::array::iter::ArrayIteratorExt;
    use vortex::error::VortexError;
    use vortex::file::OpenOptionsSessionExt;
    use vortex::io::runtime::BlockingRuntime;
    use vortex::io::runtime::current::CurrentThreadRuntime;
    use vortex::io::session::RuntimeSessionExt;
    use vortex::session::VortexSession;

    let to_polars = |e: VortexError| {
        PolarsError::ComputeError(format!("Failed to read Vortex file: {e}").into())
    };

    // A default session registers the standard encodings, layouts, and the file
    // format; `with_handle` attaches the blocking runtime the reader drives its
    // async I/O on (without it the session errors at scan time). `open_buffer`
    // then resolves segments by slicing the buffer directly, bypassing the async
    // I/O pipeline — ideal for an in-memory upload.
    let runtime = CurrentThreadRuntime::new();
    let session = <VortexSession as VortexSessionDefault>::default().with_handle(runtime.handle());
    let file = session
        .open_options()
        .open_buffer(data.to_vec())
        .map_err(to_polars)?;

    // Scan the whole file and collect every chunk into one Vortex array.
    let array = file
        .scan()
        .map_err(to_polars)?
        .into_array_iter(&runtime)
        .map_err(to_polars)?
        .read_all()
        .map_err(to_polars)?;

    // A Vortex file's root is a struct of columns; convert to an arrow-rs
    // StructArray (preferred arrow type inferred from the Vortex dtype), then
    // reinterpret it as a RecordBatch.
    let arrow_array = array.into_arrow_preferred().map_err(to_polars)?;
    let struct_array = arrow_array
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| {
            PolarsError::ComputeError("Vortex file root is not a struct of columns".into())
        })?
        .clone();
    let batch = RecordBatch::from(struct_array);

    record_batch_to_dataframe(batch)
}

/// Bridge an arrow-rs `RecordBatch` into a Polars `DataFrame` via the Arrow IPC
/// stream format — the stable interchange both Arrow implementations support.
fn record_batch_to_dataframe(batch: arrow::array::RecordBatch) -> Result<DataFrame, PolarsError> {
    use arrow::ipc::writer::StreamWriter;
    use polars::io::ipc::IpcStreamReader;

    let ipc_err =
        |e: arrow::error::ArrowError| PolarsError::ComputeError(format!("Arrow IPC: {e}").into());

    let mut buffer = Vec::new();
    {
        let mut writer = StreamWriter::try_new(&mut buffer, &batch.schema()).map_err(ipc_err)?;
        writer.write(&batch).map_err(ipc_err)?;
        writer.finish().map_err(ipc_err)?;
    }

    IpcStreamReader::new(std::io::Cursor::new(buffer)).finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::sample_frame;
    use polars::io::avro::AvroWriter;

    /// Serialize a frame to `format` bytes in memory, so the parse tests can
    /// round-trip a synthetic frame instead of reading checked-in fixtures.
    fn frame_to_bytes(df: &DataFrame, format: &str) -> Vec<u8> {
        let mut df = df.clone();
        let mut buf = Vec::new();
        match format {
            "csv" => {
                CsvWriter::new(&mut buf).finish(&mut df).unwrap();
            }
            "parquet" => {
                ParquetWriter::new(&mut buf).finish(&mut df).unwrap();
            }
            "json" => {
                JsonWriter::new(&mut buf).finish(&mut df).unwrap();
            }
            "avro" => {
                AvroWriter::new(&mut buf).finish(&mut df).unwrap();
            }
            other => panic!("unsupported test format: {other}"),
        }
        buf
    }

    /// Round-trip a synthetic frame through `parse_data` for each format. These
    /// are hermetic — the data is generated in-test, with no on-disk fixtures.
    fn assert_round_trips(format: &str, file_name: &str) {
        let df = sample_frame(100).unwrap();
        let bytes = frame_to_bytes(&df, format);
        let parsed = parse_data(&bytes, file_name).unwrap();
        assert_eq!(parsed.height(), 100);
        assert_eq!(parsed.width(), df.width());
    }

    #[test]
    fn test_parse_data_csv() {
        assert_round_trips("csv", "data.csv");
    }

    #[test]
    fn test_parse_data_parquet() {
        assert_round_trips("parquet", "data.parquet");
    }

    #[test]
    fn test_parse_data_json() {
        assert_round_trips("json", "data.json");
    }

    #[test]
    fn test_parse_data_avro() {
        assert_round_trips("avro", "data.avro");
    }

    #[test]
    fn file_type_classifies_known_extensions() {
        assert_eq!(FileType::from_file_name("data.csv"), FileType::Csv);
        assert_eq!(FileType::from_file_name("data.parquet"), FileType::Parquet);
        assert_eq!(FileType::from_file_name("data.json"), FileType::Json);
        assert_eq!(FileType::from_file_name("data.ndjson"), FileType::Json);
        assert_eq!(FileType::from_file_name("data.avro"), FileType::Avro);
    }

    #[test]
    fn file_type_is_case_insensitive() {
        assert_eq!(FileType::from_file_name("DATA.CSV"), FileType::Csv);
        assert_eq!(
            FileType::from_file_name("Report.Parquet"),
            FileType::Parquet
        );
    }

    #[test]
    fn file_type_unknown_collapses_to_other() {
        assert_eq!(FileType::from_file_name("data.xlsx"), FileType::Other);
        assert_eq!(FileType::from_file_name("noextension"), FileType::Other);
        assert_eq!(FileType::as_metric_label(FileType::Other), "other");
    }

    #[test]
    fn unsupported_file_type_is_rejected() {
        let err = parse_data(b"whatever", "data.xlsx").unwrap_err();
        assert!(matches!(err, PolarsError::ComputeError(_)));
    }

    /// Round-trip: write a Vortex file in memory, then decode it back through
    /// the public `parse_data` path. Exercises the full arrow-rs → IPC → Polars
    /// bridge against a real Vortex payload.
    #[test]
    fn test_parse_data_vortex_round_trip() {
        use vortex::VortexSessionDefault;
        use vortex::array::IntoArray;
        use vortex::array::arrays::StructArray;
        use vortex::buffer::buffer;
        use vortex::file::WriteOptionsSessionExt;
        use vortex::io::runtime::BlockingRuntime;
        use vortex::io::runtime::current::CurrentThreadRuntime;
        use vortex::io::session::RuntimeSessionExt;
        use vortex::session::VortexSession;

        let runtime = CurrentThreadRuntime::new();
        let session =
            <VortexSession as VortexSessionDefault>::default().with_handle(runtime.handle());

        let numbers = buffer![1i32, 2, 3, 4, 5].into_array();
        let array = StructArray::from_fields(&[("numbers", numbers)])
            .unwrap()
            .into_array();

        let mut bytes: Vec<u8> = Vec::new();
        session
            .write_options()
            .blocking(&runtime)
            .write(&mut bytes, array.to_array_iterator())
            .unwrap();

        let df = parse_data(&bytes, "data.vortex").unwrap();
        assert_eq!(df.height(), 5);
        assert_eq!(df.get_column_names(), ["numbers"]);
    }
}
