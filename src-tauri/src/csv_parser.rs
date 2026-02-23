// CSV Parser module — B-Stock manifest parsing and data cleaning

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::File;

#[derive(Debug, Deserialize)]
pub struct BStockManifestRow {
    #[serde(rename = "Auction name", default)]
    pub auction_name: String,

    #[serde(rename = "LotNumber", default)]
    pub lot_number: String,

    #[serde(rename = "Quantity", default)]
    pub quantity: String,

    #[serde(rename = "Title", default)]
    pub title: String,

    #[serde(rename = "Vendor Code", default)]
    pub vendor_code: Option<String>,

    #[serde(rename = "Retail Price", default)]
    pub retail_price: String,

    #[serde(rename = "Source", default)]
    pub source: Option<String>,
}

/// Parse a B-Stock manifest CSV file into structured rows
pub fn parse_bstock_csv(file_path: &str) -> Result<Vec<BStockManifestRow>, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(file);

    let mut rows = Vec::new();
    for result in reader.deserialize() {
        match result {
            Ok(row) => rows.push(row),
            Err(e) => {
                eprintln!("Warning: skipping malformed row: {}", e);
                continue;
            }
        }
    }

    Ok(rows)
}

/// Clean a price string by removing $, commas, and whitespace
pub fn clean_price(price_str: &str) -> f64 {
    price_str
        .replace('$', "")
        .replace(',', "")
        .trim()
        .parse()
        .unwrap_or(0.0)
}

/// Normalize a source string to a canonical vendor name
pub fn normalize_source(source: &Option<String>) -> String {
    match source {
        Some(s) => {
            let lower = s.to_lowercase();
            if lower.contains("best buy") || lower.contains("bestbuy") {
                "Best Buy".to_string()
            } else if lower.contains("wayfair") {
                "Wayfair".to_string()
            } else if lower.contains("mech") || lower.contains("pdx7") {
                "Mech/PDX7".to_string()
            } else if lower.contains("amazon") {
                "Amazon Bstock".to_string()
            } else {
                s.clone()
            }
        }
        None => "Unknown".to_string(),
    }
}

// ============================================================
// HiBid Results Parser
// ============================================================

#[derive(Debug, Deserialize)]
pub struct HiBidResultRow {
    #[serde(rename = "Lot #", default)]
    pub lot_number: String,

    #[serde(rename = "Winning Bidder", default)]
    pub winning_bidder: String,

    #[serde(rename = "Bidder ID", default)]
    pub bidder_id: String,

    #[serde(rename = "High Bid", default)]
    pub high_bid: String,

    #[serde(rename = "Max Bid", default)]
    pub max_bid: Option<String>,

    #[serde(rename = "Email", default)]
    pub email: Option<String>,

    #[serde(rename = "Phone", default)]
    pub phone: Option<String>,
}

/// Parse a HiBid auction results CSV
pub fn parse_hibid_results(file_path: &str) -> Result<Vec<HiBidResultRow>, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(file);

    let mut rows = Vec::new();
    for result in reader.deserialize() {
        match result {
            Ok(row) => rows.push(row),
            Err(e) => {
                eprintln!("Warning: skipping malformed result row: {}", e);
                continue;
            }
        }
    }

    Ok(rows)
}

// ============================================================
// CSV Validation
// ============================================================

#[derive(Debug, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub message: String,
    pub warnings: Vec<String>,
}

/// Validate a B-Stock CSV before import
pub fn validate_bstock_csv(path: &str) -> Result<ValidationResult, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(file);

    // 1. Check required headers
    let headers = rdr.headers().map_err(|e| e.to_string())?;
    let required = vec!["LotNumber", "Title", "Retail Price"];
    let mut missing = Vec::new();

    for req in &required {
        if !headers.iter().any(|h| h == *req) {
            missing.push(req.to_string());
        }
    }

    if !missing.is_empty() {
        return Ok(ValidationResult {
            valid: false,
            message: format!("Missing required columns: {}", missing.join(", ")),
            warnings: vec![],
        });
    }

    // 2. Validate first 10 rows
    let mut warnings = Vec::new();
    let mut row_count = 0;
    let price_col = headers.iter().position(|h| h == "Retail Price");

    for (i, result) in rdr.records().enumerate().take(10) {
        match result {
            Ok(record) => {
                row_count += 1;
                if let Some(col) = price_col {
                    if let Some(price_str) = record.get(col) {
                        let price = clean_price(price_str);
                        if price == 0.0 {
                            warnings.push(format!("Row {}: Invalid retail price", i + 2));
                        }
                    }
                }
            }
            Err(e) => {
                return Ok(ValidationResult {
                    valid: false,
                    message: format!("Invalid data at row {}: {}", i + 2, e),
                    warnings,
                });
            }
        }
    }

    Ok(ValidationResult {
        valid: true,
        message: format!("CSV is valid. Checked {} rows.", row_count),
        warnings,
    })
}

#[tauri::command]
pub fn validate_csv(file_path: String) -> Result<ValidationResult, String> {
    log::info!("Validating CSV: {}", file_path);
    validate_bstock_csv(&file_path)
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_price() {
        assert_eq!(clean_price("$1,234.56"), 1234.56);
        assert_eq!(clean_price("1234.56"), 1234.56);
        assert_eq!(clean_price("$0.99"), 0.99);
        assert_eq!(clean_price(""), 0.0);
        assert_eq!(clean_price("invalid"), 0.0);
    }

    #[test]
    fn test_normalize_source() {
        assert_eq!(normalize_source(&Some("Best Buy".to_string())), "Best Buy");
        assert_eq!(normalize_source(&Some("best buy wholesale".to_string())), "Best Buy");
        assert_eq!(normalize_source(&Some("Wayfair".to_string())), "Wayfair");
        assert_eq!(normalize_source(&Some("Mech Distribution".to_string())), "Mech/PDX7");
        assert_eq!(normalize_source(&Some("PDX7".to_string())), "Mech/PDX7");
        assert_eq!(normalize_source(&Some("Amazon B-Stock".to_string())), "Amazon Bstock");
        assert_eq!(normalize_source(&None), "Unknown");
    }
}
