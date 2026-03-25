//! crdt-merge CLI – merge, dedup, diff & json-merge from the command line.

use clap::{Parser, Subcommand};
use crdt_merge::{dedup, diff, merge, merge_json, DedupResult, DiffResult, MergeOptions, Strategy};
use crdt_merge::diff::format_diff_table;
use serde_json::Value;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process;

#[derive(Parser)]
#[command(
    name = "crdt-merge",
    about = "Conflict-free merge, dedup & diff for any dataset. Powered by CRDTs.",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Merge two CSV or JSON files
    Merge {
        /// First input file (CSV or JSON)
        file_a: String,
        /// Second input file (CSV or JSON)
        file_b: String,
        /// Key field for matching records
        #[arg(long, default_value = "id")]
        key: String,
        /// Merge strategy: lww, keep-a, keep-b
        #[arg(long, default_value = "lww")]
        strategy: String,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Deduplicate records in a CSV or JSON file
    Dedup {
        /// Input file (CSV or JSON)
        file: String,
        /// Key field for matching
        #[arg(long)]
        key: Option<String>,
        /// Fuzzy matching threshold (0.0 - 1.0)
        #[arg(long, default_value = "0.85")]
        threshold: f64,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Show structural diff between two files
    Diff {
        /// First input file (CSV or JSON)
        file_a: String,
        /// Second input file (CSV or JSON)
        file_b: String,
        /// Key field for matching records
        #[arg(long, default_value = "id")]
        key: String,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Deep-merge two JSON files
    JsonMerge {
        /// First JSON file
        file_a: String,
        /// Second JSON file
        file_b: String,
        /// Merge strategy: lww, keep-a, keep-b
        #[arg(long, default_value = "lww")]
        strategy: String,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Merge {
            file_a,
            file_b,
            key,
            strategy,
            output,
        } => cmd_merge(&file_a, &file_b, &key, &strategy, output.as_deref()),
        Commands::Dedup {
            file,
            key,
            threshold,
            output,
        } => cmd_dedup(&file, key.as_deref(), threshold, output.as_deref()),
        Commands::Diff {
            file_a,
            file_b,
            key,
            output,
        } => cmd_diff(&file_a, &file_b, &key, output.as_deref()),
        Commands::JsonMerge {
            file_a,
            file_b,
            strategy,
            output,
        } => cmd_json_merge(&file_a, &file_b, &strategy, output.as_deref()),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_strategy(s: &str) -> Result<Strategy, String> {
    match s.to_lowercase().as_str() {
        "lww" => Ok(Strategy::LWW),
        "keep-a" | "keepa" | "keep_a" => Ok(Strategy::KeepA),
        "keep-b" | "keepb" | "keep_b" => Ok(Strategy::KeepB),
        _ => Err(format!("Unknown strategy: {}", s)),
    }
}

fn read_records(path: &str) -> Result<Vec<Value>, String> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "json" => {
            let content = fs::read_to_string(path)
                .map_err(|e| format!("Cannot read {}: {}", path, e))?;
            let parsed: Value = serde_json::from_str(&content)
                .map_err(|e| format!("Invalid JSON in {}: {}", path, e))?;
            match parsed {
                Value::Array(arr) => Ok(arr),
                obj @ Value::Object(_) => Ok(vec![obj]),
                _ => Err(format!("{}: expected JSON array or object", path)),
            }
        }
        "csv" => read_csv(path),
        _ => {
            // Try JSON first, then CSV
            let content = fs::read_to_string(path)
                .map_err(|e| format!("Cannot read {}: {}", path, e))?;
            if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
                match parsed {
                    Value::Array(arr) => Ok(arr),
                    obj @ Value::Object(_) => Ok(vec![obj]),
                    _ => read_csv(path),
                }
            } else {
                read_csv(path)
            }
        }
    }
}

fn read_csv(path: &str) -> Result<Vec<Value>, String> {
    let mut rdr = csv::Reader::from_path(path)
        .map_err(|e| format!("Cannot read CSV {}: {}", path, e))?;
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| format!("CSV headers error: {}", e))?
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut records = Vec::new();
    for result in rdr.records() {
        let record = result.map_err(|e| format!("CSV record error: {}", e))?;
        let mut map = serde_json::Map::new();
        for (i, field) in record.iter().enumerate() {
            if i < headers.len() {
                // Try to parse as number or bool
                let value = if let Ok(n) = field.parse::<i64>() {
                    Value::Number(serde_json::Number::from(n))
                } else if let Ok(n) = field.parse::<f64>() {
                    serde_json::Number::from_f64(n)
                        .map(Value::Number)
                        .unwrap_or_else(|| Value::String(field.to_string()))
                } else if field == "true" {
                    Value::Bool(true)
                } else if field == "false" {
                    Value::Bool(false)
                } else {
                    Value::String(field.to_string())
                };
                map.insert(headers[i].clone(), value);
            }
        }
        records.push(Value::Object(map));
    }
    Ok(records)
}

fn write_output(content: &str, output: Option<&str>) -> Result<(), String> {
    match output {
        Some(path) => {
            let ext = Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext == "csv" {
                // Content is already CSV-formatted
                fs::write(path, content)
                    .map_err(|e| format!("Cannot write {}: {}", path, e))?;
            } else {
                fs::write(path, content)
                    .map_err(|e| format!("Cannot write {}: {}", path, e))?;
            }
        }
        None => {
            io::stdout()
                .write_all(content.as_bytes())
                .map_err(|e| format!("Write error: {}", e))?;
        }
    }
    Ok(())
}

fn records_to_csv(records: &[Value]) -> Result<String, String> {
    if records.is_empty() {
        return Ok(String::new());
    }
    // Collect all headers
    let mut headers = Vec::new();
    let mut header_set = std::collections::HashSet::new();
    for rec in records {
        if let Value::Object(map) = rec {
            for k in map.keys() {
                if header_set.insert(k.clone()) {
                    headers.push(k.clone());
                }
            }
        }
    }

    let mut wtr = csv::Writer::from_writer(Vec::new());
    wtr.write_record(&headers)
        .map_err(|e| format!("CSV write error: {}", e))?;

    for rec in records {
        let row: Vec<String> = headers
            .iter()
            .map(|h| match rec.get(h) {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::Bool(b)) => b.to_string(),
                Some(Value::Null) => String::new(),
                Some(v) => v.to_string(),
                None => String::new(),
            })
            .collect();
        wtr.write_record(&row)
            .map_err(|e| format!("CSV write error: {}", e))?;
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV flush error: {}", e))?;
    String::from_utf8(bytes).map_err(|e| format!("UTF-8 error: {}", e))
}

fn format_output(records: &[Value], output: Option<&str>) -> Result<String, String> {
    let is_csv = output
        .map(|p| {
            Path::new(p)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase()
                == "csv"
        })
        .unwrap_or(false);

    if is_csv {
        records_to_csv(records)
    } else {
        serde_json::to_string_pretty(records)
            .map_err(|e| format!("JSON serialization error: {}", e))
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn cmd_merge(
    file_a: &str,
    file_b: &str,
    key: &str,
    strategy_str: &str,
    output: Option<&str>,
) -> Result<(), String> {
    let a = read_records(file_a)?;
    let b = read_records(file_b)?;
    let strategy = parse_strategy(strategy_str)?;

    let opts = MergeOptions {
        key: key.to_string(),
        strategy,
    };

    let result = merge(&a, &b, Some(opts));
    let formatted = format_output(&result, output)?;
    write_output(&formatted, output)
}

fn cmd_dedup(
    file: &str,
    key: Option<&str>,
    threshold: f64,
    output: Option<&str>,
) -> Result<(), String> {
    let items = read_records(file)?;
    let result: DedupResult = dedup(&items, key, Some(threshold));

    eprintln!(
        "Dedup: {} records → {} unique, {} duplicates found",
        items.len(),
        result.unique.len(),
        result.duplicates.len()
    );

    let formatted = format_output(&result.unique, output)?;
    write_output(&formatted, output)
}

fn cmd_diff(
    file_a: &str,
    file_b: &str,
    key: &str,
    output: Option<&str>,
) -> Result<(), String> {
    let a = read_records(file_a)?;
    let b = read_records(file_b)?;
    let result: DiffResult = diff(&a, &b, Some(key));
    let table = format_diff_table(&result);
    write_output(&table, output)
}

fn cmd_json_merge(
    file_a: &str,
    file_b: &str,
    strategy_str: &str,
    output: Option<&str>,
) -> Result<(), String> {
    let content_a = fs::read_to_string(file_a)
        .map_err(|e| format!("Cannot read {}: {}", file_a, e))?;
    let content_b = fs::read_to_string(file_b)
        .map_err(|e| format!("Cannot read {}: {}", file_b, e))?;

    let a: Value = serde_json::from_str(&content_a)
        .map_err(|e| format!("Invalid JSON in {}: {}", file_a, e))?;
    let b: Value = serde_json::from_str(&content_b)
        .map_err(|e| format!("Invalid JSON in {}: {}", file_b, e))?;

    let strategy = parse_strategy(strategy_str)?;
    let result = merge_json(&a, &b, Some(strategy));

    let formatted = serde_json::to_string_pretty(&result)
        .map_err(|e| format!("JSON serialization error: {}", e))?;
    write_output(&formatted, output)
}
