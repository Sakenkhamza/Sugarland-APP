use crate::db::Database;
use rusqlite::Result;
use rust_xlsxwriter::{Format, Workbook, FormatAlign, FormatBorder, Color};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct Auction {
    pub id: String,
    pub hibid_auction_id: Option<String>,
    pub name: String,
    pub vendor_id: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub status: String,
    pub total_lots: i32,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateAuctionRequest {
    pub name: String,
    #[serde(default)]
    pub vendor_id: Option<String>,
    #[serde(default)]
    pub hibid_auction_id: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateVendorRequest {
    pub cost_coefficient: f64,
    pub min_price_margin: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuctionReport {
    pub id: String,
    pub auction_id: String,
    pub auction_name: String,
    pub report_type: String,
    pub file_name: String,
    pub file_path: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct FinishAuctionResult {
    pub detail_report: String,
    pub summary_report: String,
}

// Internal struct for report data
#[derive(Debug)]
struct ReportItem {
    lot_number: String,
    title: String,
    vendor_code: String,
    retail_price: f64,
    source: String,
    cost_coefficient: f64,
    cost_price: f64,
    min_price: f64,
    high_bid: f64,
    selling_price: f64,
    profit_loss: f64,
    buyer: String,
    is_buyback: bool,
    status: String,
}

pub struct AuctionManager;

impl AuctionManager {
    pub fn create_auction(db: &Database, req: CreateAuctionRequest) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        
        db.conn.execute(
            "INSERT INTO auctions (id, hibid_auction_id, name, vendor_id, start_date, end_date, status, total_lots)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'Active', 0)",
            rusqlite::params![
                id,
                req.hibid_auction_id,
                req.name,
                req.vendor_id,
                req.start_date,
                req.end_date
            ],
        )?;

        Ok(id)
    }

    pub fn list_auctions(db: &Database) -> Result<Vec<Auction>> {
        let mut stmt = db.conn.prepare(
            "SELECT id, hibid_auction_id, name, vendor_id, start_date, end_date, status, total_lots, created_at
             FROM auctions ORDER BY created_at DESC",
        )?;

        let auctions = stmt.query_map([], |row| {
            Ok(Auction {
                id: row.get(0)?,
                hibid_auction_id: row.get(1)?,
                name: row.get(2)?,
                vendor_id: row.get(3)?,
                start_date: row.get(4)?,
                end_date: row.get(5)?,
                status: row.get(6)?,
                total_lots: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

        Ok(auctions)
    }

    pub fn get_auction_by_id(db: &Database, auction_id: &str) -> Result<Auction> {
        db.conn.query_row(
            "SELECT id, hibid_auction_id, name, vendor_id, start_date, end_date, status, total_lots, created_at
             FROM auctions WHERE id = ?1",
            rusqlite::params![auction_id],
            |row| {
                Ok(Auction {
                    id: row.get(0)?,
                    hibid_auction_id: row.get(1)?,
                    name: row.get(2)?,
                    vendor_id: row.get(3)?,
                    start_date: row.get(4)?,
                    end_date: row.get(5)?,
                    status: row.get(6)?,
                    total_lots: row.get(7)?,
                    created_at: row.get(8)?,
                })
            }
        )
    }

    pub fn update_auction_status(db: &Database, auction_id: &str, status: &str) -> Result<()> {
        db.conn.execute(
            "UPDATE auctions SET status = ?1 WHERE id = ?2",
            rusqlite::params![status, auction_id],
        )?;
        Ok(())
    }

    pub fn update_vendor(db: &Database, vendor_id: &str, data: &UpdateVendorRequest) -> Result<()> {
        db.conn.execute(
            "UPDATE vendors SET cost_coefficient = ?1, min_price_margin = ?2 WHERE id = ?3",
            rusqlite::params![data.cost_coefficient, data.min_price_margin, vendor_id],
        )?;
        Ok(())
    }

    pub fn finish_auction(db: &Database, auction_id: &str, results_csv_path: &str, app_data_dir: &str) -> std::result::Result<FinishAuctionResult, String> {
        use crate::csv_parser;

        // 1. Get auction info
        let auction = Self::get_auction_by_id(db, auction_id).map_err(|e| e.to_string())?;
        if auction.status != "Active" {
            return Err("Auction is not in Active status".to_string());
        }

        // 2. Parse the HiBid results CSV
        let csv_results = csv_parser::parse_hibid_results(results_csv_path)
            .map_err(|e| format!("Failed to parse HiBid CSV: {}", e))?;

        log::info!("Parsed {} rows from HiBid results CSV", csv_results.len());

        // Build a map of lot_number -> CSV row for matching
        let mut csv_by_lot: HashMap<String, &csv_parser::HiBidResultRow> = HashMap::new();
        for row in &csv_results {
            let lot = row.lot_number.trim().to_string();
            if !lot.is_empty() {
                csv_by_lot.insert(lot, row);
            }
        }

        // 3. Get all inventory items for this auction
        let mut item_stmt = db.conn.prepare(
            "SELECT id, lot_number, raw_title, vendor_code, retail_price, source,
                    cost_price, min_price
             FROM inventory_items
             WHERE auction_id = ?1
             ORDER BY lot_number"
        ).map_err(|e| e.to_string())?;

        struct ItemInfo {
            id: String,
            lot_number: String,
            title: String,
            vendor_code: String,
            retail_price: f64,
            source: String,
            cost_price: f64,
            min_price: f64,
        }

        let db_items: Vec<ItemInfo> = item_stmt.query_map(rusqlite::params![auction_id], |row| {
            Ok(ItemInfo {
                id: row.get(0)?,
                lot_number: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                title: row.get(2)?,
                vendor_code: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                retail_price: row.get(4)?,
                source: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                cost_price: row.get(6)?,
                min_price: row.get(7)?,
            })
        }).map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;

        // 4. Drop the statement (borrow checker) before mutating db
        drop(item_stmt);

        // 5. Reconcile: match CSV results to items, update statuses, insert auction_results
        let mut report_items: Vec<ReportItem> = Vec::new();

        // First, clean up any existing auction_results for this auction
        db.conn.execute(
            "DELETE FROM auction_results WHERE auction_id = ?1",
            rusqlite::params![auction_id],
        ).map_err(|e| e.to_string())?;

        for item in &db_items {
            let csv_row = csv_by_lot.get(item.lot_number.as_str());

            let (high_bid, buyer, is_sold) = match csv_row {
                Some(row) => {
                    let bid = csv_parser::clean_price(&row.high_bid);
                    let buyer_name = row.winning_bidder.trim().to_string();
                    let bidder_id = row.bidder_id.trim().to_string();

                    // "Floor" means unsold (went to the floor with no buyer)
                    // Empty bidder_id also means unsold
                    let is_floor = buyer_name.eq_ignore_ascii_case("floor")
                        || bidder_id.is_empty()
                        || bid == 0.0;

                    if is_floor {
                        (0.0, String::new(), false)
                    } else {
                        (bid, buyer_name, true)
                    }
                }
                None => (0.0, String::new(), false),
            };

            let selling_price = high_bid / 100.0;
            
            // Re-introduce buyback logic with new names
            let buyback_names = [
                "sanzhar houston",
                "kuralay kateryne",
                "almata atastana",
                "yesset karayev"
            ];
            
            let is_buyback = is_sold && buyback_names.iter().any(|name| buyer.to_lowercase().contains(name));
            
            let profit_loss = if is_sold && !is_buyback { selling_price - item.cost_price } else { 0.0 };

            // Determine item status
            let new_status = if is_buyback {
                "Buyback"
            } else if is_sold {
                "Sold"
            } else {
                "Unsold"
            };

            // Update item status
            db.conn.execute(
                "UPDATE inventory_items SET current_status = ?1 WHERE id = ?2",
                rusqlite::params![new_status, item.id],
            ).map_err(|e| e.to_string())?;

            // Insert auction result
            let result_id = Uuid::new_v4().to_string();
            let commission = if new_status == "Sold" { selling_price * 0.15 } else { 0.0 };
            let net_profit = selling_price - item.cost_price - commission;
            db.conn.execute(
                "INSERT INTO auction_results (id, auction_id, item_id, high_bid, winning_bidder,
                 commission_amount, net_profit, is_buyback)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    result_id, auction_id, item.id,
                    selling_price, buyer,
                    commission, net_profit,
                    is_buyback
                ],
            ).map_err(|e| e.to_string())?;

            let cost_coefficient = if item.retail_price > 0.0 { item.cost_price / item.retail_price } else { 0.0 };

            report_items.push(ReportItem {
                lot_number: item.lot_number.clone(),
                title: item.title.clone(),
                vendor_code: item.vendor_code.clone(),
                retail_price: item.retail_price,
                source: item.source.clone(),
                cost_coefficient,
                cost_price: item.cost_price,
                min_price: item.min_price,
                high_bid,
                selling_price,
                profit_loss,
                buyer,
                is_buyback,
                status: new_status.to_string(),
            });
        }

        // 6. Create reports directory
        let reports_dir = format!("{}/reports/{}", app_data_dir, auction_id);
        std::fs::create_dir_all(&reports_dir).map_err(|e| format!("Failed to create reports dir: {}", e))?;

        let safe_name = auction.name.replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != ' ', "");

        // 7. Generate Report 1 (detailed per-item report)
        let detail_file_name = format!("Отчет_{}.xlsx", safe_name);
        let detail_file_path = format!("{}/{}", reports_dir, detail_file_name);
        Self::generate_detail_report(&auction.name, &report_items, &detail_file_path)?;

        // 8. Generate Report 2 (summary report)
        let summary_file_name = format!("Сводный_отчет_{}.xlsx", safe_name);
        let summary_file_path = format!("{}/{}", reports_dir, summary_file_name);
        Self::generate_summary_report(&auction.name, &report_items, &summary_file_path)?;

        // 9. Update auction status to Completed
        db.conn.execute(
            "UPDATE auctions SET status = 'Completed' WHERE id = ?1",
            rusqlite::params![auction_id],
        ).map_err(|e| e.to_string())?;

        // 10. Save report records to DB
        let detail_id = Uuid::new_v4().to_string();
        let summary_id = Uuid::new_v4().to_string();
        db.conn.execute(
            "INSERT INTO auction_reports (id, auction_id, report_type, file_name, file_path) VALUES (?1, ?2, 'detail', ?3, ?4)",
            rusqlite::params![detail_id, auction_id, detail_file_name, detail_file_path],
        ).map_err(|e| e.to_string())?;
        db.conn.execute(
            "INSERT INTO auction_reports (id, auction_id, report_type, file_name, file_path) VALUES (?1, ?2, 'summary', ?3, ?4)",
            rusqlite::params![summary_id, auction_id, summary_file_name, summary_file_path],
        ).map_err(|e| e.to_string())?;

        log::info!("Auction {} finished. Reports: {}, {}", auction_id, detail_file_name, summary_file_name);

        Ok(FinishAuctionResult {
            detail_report: detail_file_name,
            summary_report: summary_file_name,
        })
    }

    fn generate_detail_report(auction_name: &str, items: &[ReportItem], file_path: &str) -> std::result::Result<(), String> {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(auction_name.get(..31).unwrap_or(auction_name)).map_err(|e| e.to_string())?;

        // Header format
        let header_format = Format::new()
            .set_bold()
            .set_border(FormatBorder::Thin)
            .set_background_color(Color::RGB(0xD9E1F2))
            .set_align(FormatAlign::Center);

        // Data format
        let data_format = Format::new()
            .set_border(FormatBorder::Thin);

        let number_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("#,##0");

        let percent_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("0%");

        let currency_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("#,##0.00");

        // Headers
        let headers = [
            "Auction name", "LotNumber", "Quantity", "Title", "Vendor Code",
            "Retail Price", "Truckload", "cost", "cost price", "Retail price",
            "% min pr (+10%)", "min price", "High Bid", "Selling price",
            "profit/loss", "buyer"
        ];

        for (col, header) in headers.iter().enumerate() {
            worksheet.write_string_with_format(0, col as u16, *header, &header_format)
                .map_err(|e| e.to_string())?;
        }

        // Set column widths
        worksheet.set_column_width(0, 25).map_err(|e| e.to_string())?;  // Auction name
        worksheet.set_column_width(3, 45).map_err(|e| e.to_string())?;  // Title
        worksheet.set_column_width(6, 12).map_err(|e| e.to_string())?;  // Truckload
        worksheet.set_column_width(15, 20).map_err(|e| e.to_string())?; // buyer

        // Data rows
        for (i, item) in items.iter().enumerate() {
            let row = (i + 1) as u32;
            let min_pr_10 = item.retail_price * 0.10;

            worksheet.write_string_with_format(row, 0, auction_name, &data_format).map_err(|e| e.to_string())?;
            worksheet.write_string_with_format(row, 1, &item.lot_number, &data_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 2, 1.0, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_string_with_format(row, 3, &item.title, &data_format).map_err(|e| e.to_string())?;
            worksheet.write_string_with_format(row, 4, &item.vendor_code, &data_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 5, item.retail_price, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_string_with_format(row, 6, &item.source, &data_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 7, item.cost_coefficient, &percent_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 8, item.cost_price, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 9, item.retail_price, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 10, min_pr_10, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 11, item.min_price, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 12, item.high_bid, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 13, item.selling_price, &currency_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 14, item.profit_loss, &currency_format).map_err(|e| e.to_string())?;
            worksheet.write_string_with_format(row, 15, &item.buyer, &data_format).map_err(|e| e.to_string())?;
        }

        workbook.save(file_path).map_err(|e| format!("Failed to save detail report: {}", e))?;
        Ok(())
    }

    fn generate_summary_report(auction_name: &str, items: &[ReportItem], file_path: &str) -> std::result::Result<(), String> {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(auction_name.get(..31).unwrap_or(auction_name)).map_err(|e| e.to_string())?;

        // Formats
        let title_format = Format::new()
            .set_bold()
            .set_font_size(14);

        let header_format = Format::new()
            .set_bold()
            .set_border(FormatBorder::Thin)
            .set_background_color(Color::RGB(0xD9E1F2))
            .set_align(FormatAlign::Center);

        let bold_format = Format::new()
            .set_bold()
            .set_border(FormatBorder::Thin);

        let data_format = Format::new()
            .set_border(FormatBorder::Thin);

        let number_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("#,##0");

        let percent_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("0%");

        let currency_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("#,##0.00");

        // Set column widths
        worksheet.set_column_width(1, 25).map_err(|e| e.to_string())?; // Col B (Categories)
        for col in 2..=11 {
            worksheet.set_column_width(col, 15).map_err(|e| e.to_string())?; // Cols C-L
        }

        // Title Row
        let title = format!("Отчет по продажам товаров на аукционе ({})", auction_name);
        worksheet.write_string_with_format(1, 1, &title, &title_format).map_err(|e| e.to_string())?;

        // Calculate totals
        let total_count = items.len();
        let total_retail: f64 = items.iter().map(|i| i.retail_price).sum();
        let total_cost: f64 = items.iter().map(|i| i.cost_price).sum();

        let buyback_items: Vec<&ReportItem> = items.iter().filter(|i| i.is_buyback).collect();
        let buyback_count = buyback_items.len();
        let buyback_retail: f64 = buyback_items.iter().map(|i| i.retail_price).sum();
        let buyback_cost: f64 = buyback_items.iter().map(|i| i.cost_price).sum();

        let unsold_items: Vec<&ReportItem> = items.iter().filter(|i| !i.is_buyback && i.status == "Unsold").collect();
        let unsold_count = unsold_items.len();
        let unsold_retail: f64 = unsold_items.iter().map(|i| i.retail_price).sum();
        let unsold_cost: f64 = unsold_items.iter().map(|i| i.cost_price).sum();

        let sold_items: Vec<&ReportItem> = items.iter().filter(|i| i.status == "Sold").collect();
        let sold_count = sold_items.len();
        let sold_retail: f64 = sold_items.iter().map(|i| i.retail_price).sum();
        let sold_cost: f64 = sold_items.iter().map(|i| i.cost_price).sum();
        let sold_sales: f64 = sold_items.iter().map(|i| i.selling_price).sum();
        let sold_with_commission = sold_sales * 1.15;
        let sold_cost_pct = if sold_retail > 0.0 { sold_cost / sold_retail } else { 0.0 };
        let sold_sales_pct = if sold_retail > 0.0 { sold_with_commission / sold_retail } else { 0.0 };
        let sold_profit = sold_with_commission - sold_cost;

        // Group sold items by source
        let mut source_groups: HashMap<String, Vec<&ReportItem>> = HashMap::new();
        for item in &sold_items {
            source_groups.entry(item.source.clone()).or_default().push(item);
        }
        let mut sources: Vec<String> = source_groups.keys().cloned().collect();
        sources.sort();

        // Group buyback items by source
        let mut buyback_groups: HashMap<String, Vec<&ReportItem>> = HashMap::new();
        for item in &buyback_items {
            buyback_groups.entry(item.source.clone()).or_default().push(item);
        }
        let mut bb_sources: Vec<String> = buyback_groups.keys().cloned().collect();
        bb_sources.sort();

        // ============================================
        // SECTION 1: General Info
        // ============================================
        let section1_headers = ["Категории", "Кол-во", "ритейл цена", "себестоимость", "% себес.", "Дальнейшие действия"];
        for (col, h) in section1_headers.iter().enumerate() {
            worksheet.write_string_with_format(2, (col + 1) as u16, *h, &header_format).map_err(|e| e.to_string())?;
        }

        let mut row: u32 = 3;
        
        // Total
        worksheet.write_string_with_format(row, 1, "Всего", &bold_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 2, total_count as f64, &number_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 3, total_retail, &number_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 4, total_cost, &number_format).map_err(|e| e.to_string())?;
        let total_cost_pct = if total_retail > 0.0 { total_cost / total_retail } else { 0.0 };
        worksheet.write_number_with_format(row, 5, total_cost_pct, &percent_format).map_err(|e| e.to_string())?;
        row += 1;

        // Buyback
        worksheet.write_string_with_format(row, 1, "Выкуплено обратно", &bold_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 2, buyback_count as f64, &number_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 3, buyback_retail, &number_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 4, buyback_cost, &number_format).map_err(|e| e.to_string())?;
        let bb_cost_pct = if buyback_retail > 0.0 { buyback_cost / buyback_retail } else { 0.0 };
        worksheet.write_number_with_format(row, 5, bb_cost_pct, &percent_format).map_err(|e| e.to_string())?;
        row += 1;

        // Buyback source breakdown
        for source in &bb_sources {
            let group = &buyback_groups[source];
            let g_count = group.len();
            let g_retail: f64 = group.iter().map(|i| i.retail_price).sum();
            let g_cost: f64 = group.iter().map(|i| i.cost_price).sum();
            let g_cost_pct = if g_retail > 0.0 { g_cost / g_retail } else { 0.0 };

            worksheet.write_string_with_format(row, 1, source, &data_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 2, g_count as f64, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 3, g_retail, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 4, g_cost, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 5, g_cost_pct, &percent_format).map_err(|e| e.to_string())?;
            row += 1;
        }

        // Unsold
        worksheet.write_string_with_format(row, 1, "Не продано", &data_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 2, unsold_count as f64, &number_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 3, unsold_retail, &number_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 4, unsold_cost, &number_format).map_err(|e| e.to_string())?;
        row += 2; // Extra row gap

        // ============================================
        // SECTION 2: Sold on Auction
        // ============================================
        worksheet.write_string_with_format(row, 1, "Проданные на аукционе товары", &bold_format).map_err(|e| e.to_string())?;
        row += 1;
        let section2_headers = ["Категории", "Кол-во", "ритейл цена", "себестоимость", "% себес.", "Продажи", "с ком 15%", "% продажи", "прибыль/убыток"];
        for (col, h) in section2_headers.iter().enumerate() {
            worksheet.write_string_with_format(row, (col + 1) as u16, *h, &header_format).map_err(|e| e.to_string())?;
        }
        worksheet.write_string_with_format(row, 10, "% прибыли", &header_format).map_err(|e| e.to_string())?; // Col K
        row += 1;

        // Sold Total
        worksheet.write_string_with_format(row, 1, "Продано", &bold_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 2, sold_count as f64, &number_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 3, sold_retail, &number_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 4, sold_cost, &number_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 5, sold_cost_pct, &percent_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 6, sold_sales, &number_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 7, sold_with_commission, &number_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 8, sold_sales_pct, &percent_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 9, sold_profit, &currency_format).map_err(|e| e.to_string())?;
        row += 1;

        // Sold breakdown by source
        for source in &sources {
            let group = &source_groups[source];
            let g_count = group.len();
            let g_retail: f64 = group.iter().map(|i| i.retail_price).sum();
            let g_cost: f64 = group.iter().map(|i| i.cost_price).sum();
            let g_cost_pct = if g_retail > 0.0 { g_cost / g_retail } else { 0.0 };
            let g_sales: f64 = group.iter().map(|i| i.selling_price).sum();
            let g_with_comm = g_sales * 1.15;
            let g_sales_pct = if g_retail > 0.0 { g_with_comm / g_retail } else { 0.0 };
            let g_profit = g_with_comm - g_cost;
            let g_profit_pct = if sold_profit != 0.0 { g_profit / sold_profit.abs() } else { 0.0 };

            worksheet.write_string_with_format(row, 1, source, &data_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 2, g_count as f64, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 3, g_retail, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 4, g_cost, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 5, g_cost_pct, &percent_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 6, g_sales, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 7, g_with_comm, &number_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 8, g_sales_pct, &percent_format).map_err(|e| e.to_string())?;
            worksheet.write_number_with_format(row, 9, g_profit, &currency_format).map_err(|e| e.to_string())?;
            if g_profit > 0.0 {
                worksheet.write_number_with_format(row, 10, g_profit_pct, &percent_format).map_err(|e| e.to_string())?;
            }
            row += 1;
        }

        row += 2; // Extra row gap

        // ============================================
        // SECTION 3: Cash Sales
        // ============================================
        worksheet.write_string_with_format(row, 1, "Продажа наличными", &bold_format).map_err(|e| e.to_string())?;
        row += 1;
        let section3_headers = ["Категории", "кол-во", "ритейл цена", "себест.", "% себес.", "Продажи", "Клейт 10%", "% продаж", "прибыль/убыток"];
        for (col, h) in section3_headers.iter().enumerate() {
            worksheet.write_string_with_format(row, (col + 1) as u16, *h, &header_format).map_err(|e| e.to_string())?;
        }
        row += 1;
        
        // Blank row for user to fill in if needed
        worksheet.write_string_with_format(row, 1, "", &data_format).map_err(|e| e.to_string())?;
        
        row += 2; // Extra row gap
        
        // ============================================
        // SECTION 4: Overall Total
        // ============================================
        worksheet.write_string_with_format(row, 1, "Общие продажи", &bold_format).map_err(|e| e.to_string())?;
        worksheet.write_number_with_format(row, 9, sold_profit, &bold_format).map_err(|e| e.to_string())?; // Col J (Profit column)

        workbook.save(file_path).map_err(|e| format!("Failed to save summary report: {}", e))?;
        Ok(())
    }

    pub fn get_auction_reports(db: &Database, auction_id: &str) -> std::result::Result<Vec<AuctionReport>, String> {
        let mut stmt = db.conn.prepare(
            "SELECT r.id, r.auction_id, a.name, r.report_type, r.file_name, r.file_path, r.created_at
             FROM auction_reports r
             JOIN auctions a ON a.id = r.auction_id
             WHERE r.auction_id = ?1
             ORDER BY r.created_at DESC"
        ).map_err(|e| e.to_string())?;

        let reports = stmt.query_map(rusqlite::params![auction_id], |row| {
            Ok(AuctionReport {
                id: row.get(0)?,
                auction_id: row.get(1)?,
                auction_name: row.get(2)?,
                report_type: row.get(3)?,
                file_name: row.get(4)?,
                file_path: row.get(5)?,
                created_at: row.get(6)?,
            })
        }).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;

        Ok(reports)
    }

    pub fn get_all_auction_reports(db: &Database) -> std::result::Result<Vec<AuctionReport>, String> {
        let mut stmt = db.conn.prepare(
            "SELECT r.id, r.auction_id, a.name, r.report_type, r.file_name, r.file_path, r.created_at
             FROM auction_reports r
             JOIN auctions a ON a.id = r.auction_id
             ORDER BY r.created_at DESC"
        ).map_err(|e| e.to_string())?;

        let reports = stmt.query_map([], |row| {
            Ok(AuctionReport {
                id: row.get(0)?,
                auction_id: row.get(1)?,
                auction_name: row.get(2)?,
                report_type: row.get(3)?,
                file_name: row.get(4)?,
                file_path: row.get(5)?,
                created_at: row.get(6)?,
            })
        }).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;

        Ok(reports)
    }
}

// Tauri Commands

#[tauri::command]
pub fn create_auction(
    req: CreateAuctionRequest,
    state: State<crate::AppState>,
) -> std::result::Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::create_auction(&db, req).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_auctions(state: State<crate::AppState>) -> std::result::Result<Vec<Auction>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::list_auctions(&db).map_err(|e| e.to_string())
}


#[tauri::command]
pub fn get_auction_by_id(
    auction_id: String,
    state: State<crate::AppState>,
) -> std::result::Result<Auction, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::get_auction_by_id(&db, &auction_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_auction_status(
    auction_id: String,
    status: String,
    state: State<crate::AppState>,
) -> std::result::Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::update_auction_status(&db, &auction_id, &status).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_vendor(
    vendor_id: String,
    data: UpdateVendorRequest,
    state: State<crate::AppState>,
) -> std::result::Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::update_vendor(&db, &vendor_id, &data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn unassign_item(
    item_id: String,
    state: State<crate::AppState>,
) -> std::result::Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    
    // Get auction_id before reset
    let auction_id: Option<String> = db.conn.query_row(
        "SELECT auction_id FROM inventory_items WHERE id = ?1",
        rusqlite::params![item_id],
        |r| r.get(0),
    ).unwrap_or(None);

    // Reset status and auction_id
    db.conn.execute(
        "UPDATE inventory_items SET current_status = 'InStock', auction_id = NULL, listed_at = NULL WHERE id = ?1",
        rusqlite::params![item_id],
    ).map_err(|e| e.to_string())?;

    // Update auction total_lots count
    if let Some(auc_id) = auction_id {
        db.conn.execute(
            "UPDATE auctions SET total_lots = (
                SELECT COUNT(*) FROM inventory_items WHERE auction_id = ?1
             ) WHERE id = ?1",
            rusqlite::params![auc_id],
        ).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn finish_auction(
    auction_id: String,
    results_csv_path: String,
    app_handle: tauri::AppHandle,
    state: State<crate::AppState>,
) -> std::result::Result<FinishAuctionResult, String> {
    use tauri::Manager;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    
    // Get app data directory (Tauri 2 API)
    let app_data_dir = app_handle.path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?
        .to_string_lossy()
        .to_string();
    
    AuctionManager::finish_auction(&db, &auction_id, &results_csv_path, &app_data_dir)
}

#[tauri::command]
pub fn get_auction_reports(
    auction_id: String,
    state: State<crate::AppState>,
) -> std::result::Result<Vec<AuctionReport>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::get_auction_reports(&db, &auction_id)
}

#[tauri::command]
pub fn get_all_auction_reports(
    state: State<crate::AppState>,
) -> std::result::Result<Vec<AuctionReport>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::get_all_auction_reports(&db)
}

#[tauri::command]
pub fn open_report_file(file_path: String) -> std::result::Result<(), String> {
    // Open file with default system application
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &file_path])
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    Ok(())
}
