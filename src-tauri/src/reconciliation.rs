use crate::csv_parser;
use crate::db::Database;
use rusqlite::{params, Result};
use serde::Serialize;
use tauri::State;

const ITEM_STATUS_SQL: &str = "COALESCE(
    ar.item_status,
    CASE
        WHEN ar.is_buyback = TRUE THEN 'Buyback'
        WHEN COALESCE(ar.high_bid, 0) > 0 THEN 'Sold'
        ELSE 'Unsold'
    END
)";

#[derive(Debug, Serialize)]
pub struct ReconciliationResult {
    pub sold_count: i32,
    pub buyback_count: i32,
    pub total_revenue: f64,
    pub total_profit: f64,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ProfitLossReport {
    pub total_revenue: f64,
    pub total_cogs: f64,
    pub gross_profit: f64,
    pub total_expenses: f64,
    pub net_profit: f64,
    pub margin_percent: f64,
    pub sold_items: i32,
    pub total_lots: i32,
    pub buyback_count: i32,
    pub unsold_count: i32,
    pub sell_through_rate: f64,
    pub avg_sale_price: f64,
    pub period_label: String,
}

#[derive(Debug, Serialize)]
pub struct AuctionSummary {
    pub auction_id: String,
    pub auction_name: String,
    pub completed_at: String,
    pub total_lots: i32,
    pub sold_count: i32,
    pub buyback_count: i32,
    pub unsold_count: i32,
    pub total_revenue: f64,
    pub total_cogs: f64,
    pub total_commission: f64,
    pub net_profit: f64,
    pub margin_percent: f64,
}

#[derive(Debug, Serialize)]
pub struct VendorBreakdown {
    pub source: String,
    pub item_count: i32,
    pub total_retail: f64,
    pub total_cost: f64,
    pub cost_pct: f64,
    pub total_revenue: f64,
    pub revenue_with_commission: f64,
    pub revenue_pct: f64,
    pub profit_loss: f64,
}

struct PeriodFilter {
    clause: String,
    custom_range: Option<(String, String)>,
    label: String,
}

fn build_period_filter(
    period: &str,
    date_from: Option<String>,
    date_to: Option<String>,
    column: &str,
) -> Result<PeriodFilter, String> {
    let normalized = period.trim().to_lowercase();
    match normalized.as_str() {
        "week" => Ok(PeriodFilter {
            clause: format!(" AND {column} >= datetime('now', '-7 days')"),
            custom_range: None,
            label: "Last 7 days".to_string(),
        }),
        "month" => Ok(PeriodFilter {
            clause: format!(" AND {column} >= datetime('now', '-1 month')"),
            custom_range: None,
            label: "Last 30 days".to_string(),
        }),
        "all" | "" => Ok(PeriodFilter {
            clause: String::new(),
            custom_range: None,
            label: "All time".to_string(),
        }),
        "custom" => {
            let from = date_from
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or_else(|| "date_from is required for custom period".to_string())?;
            let to = date_to
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or_else(|| "date_to is required for custom period".to_string())?;
            Ok(PeriodFilter {
                clause: format!(
                    " AND {column} >= datetime(?1) AND {column} < datetime(?2, '+1 day')"
                ),
                custom_range: Some((from.clone(), to.clone())),
                label: format!("{from} - {to}"),
            })
        }
        _ => Err(format!("Unsupported period: {period}")),
    }
}

pub struct ReconciliationManager;

impl ReconciliationManager {
    pub fn reconcile_hibid_results(
        db: &Database,
        auction_id: &str,
        file_path: &str,
    ) -> Result<ReconciliationResult, String> {
        let results = csv_parser::parse_hibid_results(file_path).map_err(|e| e.to_string())?;

        let mut sold_count = 0;
        let mut buyback_count = 0;
        let mut total_revenue = 0.0;
        let mut total_profit = 0.0;
        let mut errors = Vec::new();

        let tx = db.conn.unchecked_transaction().map_err(|e| e.to_string())?;

        let mut bb_stmt = db
            .conn
            .prepare("SELECT name FROM buybackers WHERE is_active = 1")
            .map_err(|e| e.to_string())?;
        let buyback_names: Vec<String> = bb_stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .map(|n| n.trim().to_lowercase())
            .collect();

        let legacy_id: String = db
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'ron_larsson_bidder_id'",
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "5046".to_string());

        let commission_rate: f64 = db
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'default_commission_rate'",
                [],
                |row| {
                    let val: String = row.get(0)?;
                    Ok(val.parse::<f64>().unwrap_or(0.15))
                },
            )
            .unwrap_or(0.15);

        for row in results {
            let high_bid = csv_parser::clean_hibid_cents_price(&row.high_bid);
            let max_bid = row
                .max_bid
                .as_deref()
                .map(csv_parser::clean_hibid_cents_price)
                .unwrap_or(0.0);

            let has_valid_bidder = !row.winning_bidder.trim().is_empty()
                && !row.winning_bidder.trim().eq_ignore_ascii_case("floor")
                && high_bid > 0.0;

            let winner_lower = row.winning_bidder.to_lowercase();
            let is_buyback_detected = buyback_names
                .iter()
                .any(|bb_name| winner_lower.contains(bb_name))
                || row.bidder_id == legacy_id;

            let item_data: rusqlite::Result<(String, f64, f64)> = tx.query_row(
                "SELECT id, cost_price, min_price
                 FROM inventory_items
                 WHERE lot_number = ?1
                   AND auction_id = ?2
                   AND current_status = 'Listed'",
                params![row.lot_number, auction_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            );

            let (item_id, cost, min_price_snapshot) = match item_data {
                Ok(data) => data,
                Err(_) => {
                    errors.push(format!(
                        "Lot {}: Item not found or not listed in this auction",
                        row.lot_number
                    ));
                    continue;
                }
            };

            let is_buyback = has_valid_bidder && is_buyback_detected;
            let status = if is_buyback {
                "Buyback"
            } else if has_valid_bidder {
                "Sold"
            } else {
                "Unsold"
            };

            let commission = if status == "Sold" {
                high_bid * commission_rate
            } else {
                0.0
            };
            let net_profit = if status == "Sold" {
                high_bid - cost - commission
            } else {
                0.0
            };

            tx.execute(
                "UPDATE inventory_items
                 SET current_status = ?1,
                     sold_at = CASE WHEN ?1 = 'Sold' THEN CURRENT_TIMESTAMP ELSE NULL END
                 WHERE id = ?2",
                params![status, item_id],
            )
            .map_err(|e| e.to_string())?;

            tx.execute(
                "INSERT INTO auction_results
                 (id, auction_id, item_id, winning_bidder, bidder_id, high_bid, max_bid,
                  is_buyback, commission_rate, commission_amount, net_profit, item_status, min_price_snapshot)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    auction_id,
                    item_id,
                    row.winning_bidder,
                    row.bidder_id,
                    high_bid,
                    max_bid,
                    is_buyback,
                    commission_rate,
                    commission,
                    net_profit,
                    status,
                    min_price_snapshot
                ],
            )
            .map_err(|e| e.to_string())?;

            match status {
                "Sold" => {
                    sold_count += 1;
                    total_revenue += high_bid;
                    total_profit += net_profit;
                }
                "Buyback" => {
                    buyback_count += 1;
                }
                _ => {}
            }
        }

        tx.commit().map_err(|e| e.to_string())?;

        db.conn
            .execute(
                "UPDATE auctions SET status = 'Completed' WHERE id = ?1",
                params![auction_id],
            )
            .map_err(|e| e.to_string())?;

        Ok(ReconciliationResult {
            sold_count,
            buyback_count,
            total_revenue,
            total_profit,
            errors,
        })
    }

    pub fn generate_pl_report(db: &Database) -> Result<ProfitLossReport, String> {
        Self::generate_pl_report_filtered(db, "all".to_string(), None, None)
    }

    pub fn generate_pl_report_filtered(
        db: &Database,
        period: String,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<ProfitLossReport, String> {
        let filter = build_period_filter(&period, date_from, date_to, "ar.created_at")?;
        let sql = format!(
            "
            SELECT
                COUNT(*) as total_lots,
                SUM(CASE WHEN {status_sql} = 'Sold' THEN 1 ELSE 0 END) as sold_items,
                SUM(CASE WHEN {status_sql} = 'Buyback' THEN 1 ELSE 0 END) as buyback_count,
                SUM(CASE WHEN {status_sql} = 'Unsold' THEN 1 ELSE 0 END) as unsold_count,
                COALESCE(SUM(CASE WHEN {status_sql} = 'Sold' THEN ar.high_bid ELSE 0 END), 0) as revenue,
                COALESCE(SUM(CASE WHEN {status_sql} = 'Sold' THEN i.cost_price ELSE 0 END), 0) as cogs,
                COALESCE(SUM(CASE WHEN {status_sql} = 'Sold' THEN ar.commission_amount ELSE 0 END), 0) as expenses,
                COALESCE(SUM(CASE WHEN {status_sql} = 'Sold' THEN ar.net_profit ELSE 0 END), 0) as net_profit
            FROM auction_results ar
            JOIN inventory_items i ON ar.item_id = i.id
            WHERE 1 = 1
            {filter_clause}
            ",
            status_sql = ITEM_STATUS_SQL,
            filter_clause = filter.clause
        );

        let mapper = |row: &rusqlite::Row<'_>| -> rusqlite::Result<(i64, i64, i64, i64, f64, f64, f64, f64)> {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        };

        let (
            total_lots,
            sold_items,
            buyback_count,
            unsold_count,
            revenue,
            cogs,
            expenses,
            net_profit,
        ) = if let Some((from, to)) = &filter.custom_range {
            db.conn
                .query_row(&sql, params![from, to], mapper)
                .map_err(|e| e.to_string())?
        } else {
            db.conn
                .query_row(&sql, [], mapper)
                .map_err(|e| e.to_string())?
        };

        let gross_profit = revenue - cogs;
        let margin_percent = if revenue > 0.0 {
            (net_profit / revenue) * 100.0
        } else {
            0.0
        };
        let sell_through_base = sold_items + unsold_count;
        let sell_through_rate = if sell_through_base > 0 {
            (sold_items as f64 / sell_through_base as f64) * 100.0
        } else {
            0.0
        };
        let avg_sale_price = if sold_items > 0 {
            revenue / sold_items as f64
        } else {
            0.0
        };

        Ok(ProfitLossReport {
            total_revenue: revenue,
            total_cogs: cogs,
            gross_profit,
            total_expenses: expenses,
            net_profit,
            margin_percent,
            sold_items: sold_items as i32,
            total_lots: total_lots as i32,
            buyback_count: buyback_count as i32,
            unsold_count: unsold_count as i32,
            sell_through_rate,
            avg_sale_price,
            period_label: filter.label,
        })
    }

    pub fn get_auction_summaries(
        db: &Database,
        period: String,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<Vec<AuctionSummary>, String> {
        let filter = build_period_filter(&period, date_from, date_to, "ar.created_at")?;
        let sql = format!(
            "
            SELECT
                a.id,
                a.name,
                COALESCE(a.end_date, MAX(ar.created_at), a.created_at) as completed_at,
                COUNT(*) as total_lots,
                SUM(CASE WHEN {status_sql} = 'Sold' THEN 1 ELSE 0 END) as sold_count,
                SUM(CASE WHEN {status_sql} = 'Buyback' THEN 1 ELSE 0 END) as buyback_count,
                SUM(CASE WHEN {status_sql} = 'Unsold' THEN 1 ELSE 0 END) as unsold_count,
                COALESCE(SUM(CASE WHEN {status_sql} = 'Sold' THEN ar.high_bid ELSE 0 END), 0) as revenue,
                COALESCE(SUM(CASE WHEN {status_sql} = 'Sold' THEN i.cost_price ELSE 0 END), 0) as cogs,
                COALESCE(SUM(CASE WHEN {status_sql} = 'Sold' THEN ar.commission_amount ELSE 0 END), 0) as commission,
                COALESCE(SUM(CASE WHEN {status_sql} = 'Sold' THEN ar.net_profit ELSE 0 END), 0) as net_profit
            FROM auctions a
            JOIN auction_results ar ON ar.auction_id = a.id
            JOIN inventory_items i ON ar.item_id = i.id
            WHERE a.status = 'Completed'
            {filter_clause}
            GROUP BY a.id, a.name, a.end_date, a.created_at
            ORDER BY datetime(completed_at) DESC
            ",
            status_sql = ITEM_STATUS_SQL,
            filter_clause = filter.clause
        );

        let mut stmt = db.conn.prepare(&sql).map_err(|e| e.to_string())?;
        let mapper = |row: &rusqlite::Row<'_>| -> rusqlite::Result<AuctionSummary> {
            let total_revenue: f64 = row.get(7)?;
            let net_profit: f64 = row.get(10)?;
            let margin_percent = if total_revenue > 0.0 {
                (net_profit / total_revenue) * 100.0
            } else {
                0.0
            };
            Ok(AuctionSummary {
                auction_id: row.get(0)?,
                auction_name: row.get(1)?,
                completed_at: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                total_lots: row.get::<_, i64>(3)? as i32,
                sold_count: row.get::<_, i64>(4)? as i32,
                buyback_count: row.get::<_, i64>(5)? as i32,
                unsold_count: row.get::<_, i64>(6)? as i32,
                total_revenue,
                total_cogs: row.get(8)?,
                total_commission: row.get(9)?,
                net_profit,
                margin_percent,
            })
        };

        let rows = if let Some((from, to)) = &filter.custom_range {
            stmt.query_map(params![from, to], mapper)
        } else {
            stmt.query_map([], mapper)
        }
        .map_err(|e| e.to_string())?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())
    }

    pub fn get_vendor_breakdown(
        db: &Database,
        period: String,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<Vec<VendorBreakdown>, String> {
        let filter = build_period_filter(&period, date_from, date_to, "ar.created_at")?;
        let sql = format!(
            "
            SELECT
                COALESCE(NULLIF(TRIM(i.source), ''), 'Unknown') as source,
                COUNT(*) as item_count,
                COALESCE(SUM(i.retail_price), 0) as total_retail,
                COALESCE(SUM(i.cost_price), 0) as total_cost,
                COALESCE(SUM(ar.high_bid), 0) as revenue,
                COALESCE(SUM(ar.high_bid * 1.15), 0) as revenue_with_comm,
                COALESCE(SUM(ar.net_profit), 0) as profit_loss
            FROM auction_results ar
            JOIN inventory_items i ON ar.item_id = i.id
            WHERE {status_sql} = 'Sold'
            {filter_clause}
            GROUP BY source
            ORDER BY revenue DESC
            ",
            status_sql = ITEM_STATUS_SQL,
            filter_clause = filter.clause
        );

        let mut stmt = db.conn.prepare(&sql).map_err(|e| e.to_string())?;
        let mapper = |row: &rusqlite::Row<'_>| -> rusqlite::Result<VendorBreakdown> {
            let total_retail: f64 = row.get(2)?;
            let total_cost: f64 = row.get(3)?;
            let total_revenue: f64 = row.get(4)?;
            let revenue_with_commission: f64 = row.get(5)?;
            let cost_pct = if total_retail > 0.0 {
                (total_cost / total_retail) * 100.0
            } else {
                0.0
            };
            let revenue_pct = if total_retail > 0.0 {
                (revenue_with_commission / total_retail) * 100.0
            } else {
                0.0
            };
            Ok(VendorBreakdown {
                source: row.get(0)?,
                item_count: row.get::<_, i64>(1)? as i32,
                total_retail,
                total_cost,
                cost_pct,
                total_revenue,
                revenue_with_commission,
                revenue_pct,
                profit_loss: row.get(6)?,
            })
        };

        let rows = if let Some((from, to)) = &filter.custom_range {
            stmt.query_map(params![from, to], mapper)
        } else {
            stmt.query_map([], mapper)
        }
        .map_err(|e| e.to_string())?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn reconcile_auction(
    auction_id: String,
    file_path: String,
    state: State<crate::AppState>,
) -> Result<ReconciliationResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    ReconciliationManager::reconcile_hibid_results(&db, &auction_id, &file_path)
}

#[tauri::command]
pub fn get_pl_report(state: State<crate::AppState>) -> Result<ProfitLossReport, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    ReconciliationManager::generate_pl_report(&db)
}

#[tauri::command]
pub fn get_pl_report_filtered(
    period: String,
    date_from: Option<String>,
    date_to: Option<String>,
    state: State<crate::AppState>,
) -> Result<ProfitLossReport, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    ReconciliationManager::generate_pl_report_filtered(&db, period, date_from, date_to)
}

#[tauri::command]
pub fn get_auction_summaries(
    period: String,
    date_from: Option<String>,
    date_to: Option<String>,
    state: State<crate::AppState>,
) -> Result<Vec<AuctionSummary>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    ReconciliationManager::get_auction_summaries(&db, period, date_from, date_to)
}

#[tauri::command]
pub fn get_vendor_breakdown(
    period: String,
    date_from: Option<String>,
    date_to: Option<String>,
    state: State<crate::AppState>,
) -> Result<Vec<VendorBreakdown>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    ReconciliationManager::get_vendor_breakdown(&db, period, date_from, date_to)
}
