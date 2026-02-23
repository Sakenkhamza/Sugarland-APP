// Database module — SQLite management with migrations and queries

use rusqlite::{Connection, Result};
use serde::Serialize;

pub struct Database {
    pub conn: Connection,
}

#[derive(Debug, Serialize, Clone)]
pub struct InventoryItemRow {
    pub id: String,
    pub manifest_id: String,
    pub lot_number: Option<String>,
    pub quantity: i32,
    pub raw_title: String,
    pub vendor_code: Option<String>,
    pub source: Option<String>,
    pub condition: Option<String>,
    pub normalized_title: Option<String>,
    pub extracted_brand: Option<String>,
    pub extracted_model: Option<String>,
    pub sku_extracted: Option<String>,
    pub category: Option<String>,
    pub retail_price: f64,
    pub cost_price: f64,
    pub min_price: f64,
    pub current_status: String,
    pub auction_id: Option<String>,
    pub listed_at: Option<String>,
    pub sold_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub total_items: i64,
    pub in_stock: i64,
    pub listed: i64,
    pub sold: i64,
    pub buyback: i64,
    pub total_retail_value: f64,
    pub total_cost: f64,
    pub active_auctions: i64,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Performance pragmas
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;
             PRAGMA synchronous=NORMAL;
             PRAGMA cache_size=-64000;",
        )?;

        let db = Self { conn };
        db.run_migrations()?;
        db.seed_vendors()?;

        Ok(db)
    }

    fn run_migrations(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            -- Vendors (supplier reference)
            CREATE TABLE IF NOT EXISTS vendors (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                cost_coefficient REAL NOT NULL CHECK(cost_coefficient > 0 AND cost_coefficient < 1),
                min_price_margin REAL NOT NULL DEFAULT 0.10,
                is_active BOOLEAN DEFAULT TRUE,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            -- Manifests (B-Stock shipments)
            CREATE TABLE IF NOT EXISTS manifests (
                id TEXT PRIMARY KEY,
                import_date DATETIME DEFAULT CURRENT_TIMESTAMP,
                source_filename TEXT NOT NULL,
                total_retail_value REAL,
                total_cost REAL,
                items_count INTEGER,
                status TEXT CHECK(status IN ('Imported', 'Listed', 'Closed')) DEFAULT 'Imported',
                notes TEXT
            );

            -- Auctions (HiBid auctions)
            CREATE TABLE IF NOT EXISTS auctions (
                id TEXT PRIMARY KEY,
                hibid_auction_id TEXT UNIQUE,
                name TEXT NOT NULL,
                vendor_id TEXT,
                start_date DATETIME,
                end_date DATETIME,
                status TEXT CHECK(status IN ('Draft', 'Active', 'Completed', 'Cancelled')) DEFAULT 'Draft',
                total_lots INTEGER DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            -- Inventory Items
            CREATE TABLE IF NOT EXISTS inventory_items (
                id TEXT PRIMARY KEY,
                manifest_id TEXT NOT NULL REFERENCES manifests(id) ON DELETE CASCADE,
                lot_number TEXT,
                quantity INTEGER DEFAULT 1,

                -- Raw data
                raw_title TEXT NOT NULL,
                vendor_code TEXT,
                source TEXT,
                condition TEXT,

                -- Normalized data
                normalized_title TEXT,
                extracted_brand TEXT,
                extracted_model TEXT,
                sku_extracted TEXT,
                category TEXT,

                -- Financial
                retail_price REAL NOT NULL CHECK(retail_price >= 0),
                cost_price REAL NOT NULL CHECK(cost_price >= 0),
                min_price REAL NOT NULL CHECK(min_price >= 0),

                -- Status
                current_status TEXT CHECK(current_status IN ('InStock', 'Listed', 'Sold', 'Buyback', 'Scrap')) DEFAULT 'InStock',

                -- Auction
                auction_id TEXT REFERENCES auctions(id),
                listed_at DATETIME,
                sold_at DATETIME,

                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            -- Auction Results
            CREATE TABLE IF NOT EXISTS auction_results (
                id TEXT PRIMARY KEY,
                auction_id TEXT NOT NULL REFERENCES auctions(id),
                item_id TEXT NOT NULL REFERENCES inventory_items(id),

                winning_bidder TEXT,
                bidder_id TEXT,
                high_bid REAL NOT NULL,
                max_bid REAL,

                bidder_email TEXT,
                bidder_phone TEXT,

                is_buyback BOOLEAN DEFAULT FALSE,
                is_paid BOOLEAN DEFAULT FALSE,

                commission_rate REAL DEFAULT 0.15,
                commission_amount REAL,
                net_profit REAL,

                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,

                UNIQUE(auction_id, item_id)
            );

            -- Historical Sales (for ML pipeline)
            CREATE TABLE IF NOT EXISTS historical_sales (
                id TEXT PRIMARY KEY,
                normalized_title TEXT NOT NULL,
                extracted_brand TEXT,
                extracted_sku TEXT,
                category TEXT,
                condition TEXT,

                retail_price REAL,
                cost_price REAL,
                sale_price REAL NOT NULL,

                sale_date DATE NOT NULL,
                platform TEXT DEFAULT 'HiBid',
                season TEXT,

                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            -- Indexes
            CREATE INDEX IF NOT EXISTS idx_inventory_status ON inventory_items(current_status);
            CREATE INDEX IF NOT EXISTS idx_inventory_manifest ON inventory_items(manifest_id);
            CREATE INDEX IF NOT EXISTS idx_inventory_auction ON inventory_items(auction_id);
            CREATE INDEX IF NOT EXISTS idx_auction_results_auction ON auction_results(auction_id);
            CREATE INDEX IF NOT EXISTS idx_auction_results_buyback ON auction_results(is_buyback);
            CREATE INDEX IF NOT EXISTS idx_historical_brand ON historical_sales(extracted_brand);
            CREATE INDEX IF NOT EXISTS idx_historical_date ON historical_sales(sale_date);

            -- Trigger: auto-populate historical_sales on auction result insertion
            CREATE TRIGGER IF NOT EXISTS after_auction_result_insert
            AFTER INSERT ON auction_results
            FOR EACH ROW
            WHEN NEW.is_buyback = FALSE
            BEGIN
                INSERT OR IGNORE INTO historical_sales (
                    id, normalized_title, extracted_brand, extracted_sku,
                    category, condition, retail_price, cost_price,
                    sale_price, sale_date, platform, season
                )
                SELECT
                    NEW.id,
                    i.normalized_title,
                    i.extracted_brand,
                    i.sku_extracted,
                    i.category,
                    i.condition,
                    i.retail_price,
                    i.cost_price,
                    NEW.high_bid,
                    DATE('now'),
                    'HiBid',
                    CASE
                        WHEN CAST(strftime('%m', 'now') AS INTEGER) BETWEEN 1 AND 3 THEN 'Q1'
                        WHEN CAST(strftime('%m', 'now') AS INTEGER) BETWEEN 4 AND 6 THEN 'Q2'
                        WHEN CAST(strftime('%m', 'now') AS INTEGER) BETWEEN 7 AND 9 THEN 'Q3'
                        ELSE 'Q4'
                    END
                FROM inventory_items i
                WHERE i.id = NEW.item_id;
            END;

            -- Settings table for runtime configuration
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                description TEXT,
                category TEXT DEFAULT 'general',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            -- Additional indexes for NLP fields
            CREATE INDEX IF NOT EXISTS idx_inventory_brand ON inventory_items(extracted_brand);
            CREATE INDEX IF NOT EXISTS idx_inventory_model ON inventory_items(extracted_model);
            CREATE INDEX IF NOT EXISTS idx_inventory_sku ON inventory_items(sku_extracted);
            CREATE INDEX IF NOT EXISTS idx_inventory_category ON inventory_items(category);
            CREATE INDEX IF NOT EXISTS idx_settings_category ON settings(category);
            CREATE INDEX IF NOT EXISTS idx_historical_sku ON historical_sales(extracted_sku);
            CREATE INDEX IF NOT EXISTS idx_historical_season ON historical_sales(season);
            CREATE INDEX IF NOT EXISTS idx_historical_category ON historical_sales(category);

            -- View: sales analytics by brand/category/season
            CREATE VIEW IF NOT EXISTS v_sales_analytics AS
            SELECT
                hs.extracted_brand,
                hs.category,
                hs.season,
                COUNT(*) as sales_count,
                AVG(hs.sale_price) as avg_sale_price,
                AVG(hs.retail_price) as avg_retail_price,
                MIN(hs.sale_date) as first_sale,
                MAX(hs.sale_date) as last_sale
            FROM historical_sales hs
            GROUP BY hs.extracted_brand, hs.category, hs.season;

            -- View: P&L per auction
            CREATE VIEW IF NOT EXISTS v_auction_pnl AS
            SELECT
                a.id as auction_id,
                a.name as auction_name,
                a.start_date,
                a.end_date,
                COUNT(DISTINCT i.id) as total_items,
                SUM(CASE WHEN ar.is_buyback = FALSE THEN 1 ELSE 0 END) as sold_items,
                SUM(CASE WHEN ar.is_buyback = TRUE THEN 1 ELSE 0 END) as buyback_items,
                SUM(CASE WHEN ar.is_buyback = FALSE THEN ar.high_bid ELSE 0 END) as total_revenue,
                SUM(CASE WHEN ar.is_buyback = FALSE THEN i.cost_price ELSE 0 END) as total_cost,
                SUM(CASE WHEN ar.is_buyback = FALSE THEN ar.commission_amount ELSE 0 END) as total_commission,
                SUM(CASE WHEN ar.is_buyback = FALSE THEN ar.net_profit ELSE 0 END) as net_profit
            FROM auctions a
            LEFT JOIN inventory_items i ON i.auction_id = a.id
            LEFT JOIN auction_results ar ON ar.auction_id = a.id AND ar.item_id = i.id
            GROUP BY a.id;

            -- Trigger: auto-update updated_at on inventory_items
            CREATE TRIGGER IF NOT EXISTS update_inventory_timestamp
            AFTER UPDATE ON inventory_items
            FOR EACH ROW
            BEGIN
                UPDATE inventory_items
                SET updated_at = CURRENT_TIMESTAMP
                WHERE id = NEW.id;
            END;

            -- Trigger: auto-update updated_at on settings
            CREATE TRIGGER IF NOT EXISTS update_settings_timestamp
            AFTER UPDATE ON settings
            FOR EACH ROW
            BEGIN
                UPDATE settings
                SET updated_at = CURRENT_TIMESTAMP
                WHERE key = NEW.key;
            END;
            ",
        )?;

        // Seed settings
        self.conn.execute_batch(
            "
            INSERT OR IGNORE INTO settings (key, value, description, category) VALUES
                ('ron_larsson_bidder_id', '5046', 'Internal buyback bidder ID (Ron Larsson)', 'reconciliation');
            INSERT OR IGNORE INTO settings (key, value, description, category) VALUES
                ('default_commission_rate', '0.15', 'Default auction commission rate (15%)', 'financial');
            INSERT OR IGNORE INTO settings (key, value, description, category) VALUES
                ('cash_sale_commission_rate', '0.10', 'Commission rate for cash sales (10%)', 'financial');
            INSERT OR IGNORE INTO settings (key, value, description, category) VALUES
                ('app_version', '0.2.0', 'Current application version', 'system');
            INSERT OR IGNORE INTO settings (key, value, description, category) VALUES
                ('db_version', '2', 'Database schema version', 'system');
            ",
        )?;

        Ok(())
    }

    fn seed_vendors(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            INSERT OR IGNORE INTO vendors (id, name, cost_coefficient, min_price_margin) VALUES
                ('bestbuy', 'Best Buy', 0.14, 0.10);
            INSERT OR IGNORE INTO vendors (id, name, cost_coefficient, min_price_margin) VALUES
                ('wayfair', 'Wayfair', 0.07, 0.10);
            INSERT OR IGNORE INTO vendors (id, name, cost_coefficient, min_price_margin) VALUES
                ('mech', 'Mech/PDX7', 0.20, 0.10);
            INSERT OR IGNORE INTO vendors (id, name, cost_coefficient, min_price_margin) VALUES
                ('amazon', 'Amazon Bstock', 0.20, 0.10);
            ",
        )?;

        Ok(())
    }

    pub fn get_inventory_items(&self, status: Option<&str>) -> Result<Vec<InventoryItemRow>> {
        let mut query = String::from(
            "SELECT id, manifest_id, lot_number, quantity,
                    raw_title, vendor_code, source, condition,
                    normalized_title, extracted_brand, extracted_model, sku_extracted, category,
                    retail_price, cost_price, min_price,
                    current_status, auction_id, listed_at, sold_at,
                    created_at, updated_at
             FROM inventory_items WHERE 1=1",
        );

        if let Some(s) = status {
            query.push_str(&format!(" AND current_status = '{}'", s));
        }

        query.push_str(" ORDER BY created_at DESC LIMIT 1000");

        let mut stmt = self.conn.prepare(&query)?;
        let items = stmt
            .query_map([], |row| {
                Ok(InventoryItemRow {
                    id: row.get(0)?,
                    manifest_id: row.get(1)?,
                    lot_number: row.get(2)?,
                    quantity: row.get(3)?,
                    raw_title: row.get(4)?,
                    vendor_code: row.get(5)?,
                    source: row.get(6)?,
                    condition: row.get(7)?,
                    normalized_title: row.get(8)?,
                    extracted_brand: row.get(9)?,
                    extracted_model: row.get(10)?,
                    sku_extracted: row.get(11)?,
                    category: row.get(12)?,
                    retail_price: row.get(13)?,
                    cost_price: row.get(14)?,
                    min_price: row.get(15)?,
                    current_status: row.get(16)?,
                    auction_id: row.get(17)?,
                    listed_at: row.get(18)?,
                    sold_at: row.get(19)?,
                    created_at: row.get(20)?,
                    updated_at: row.get(21)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(items)
    }

    pub fn get_dashboard_stats(&self) -> Result<DashboardStats> {
        let total_items: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM inventory_items", [], |r| r.get(0))?;

        let in_stock: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM inventory_items WHERE current_status = 'InStock'",
            [],
            |r| r.get(0),
        )?;

        let listed: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM inventory_items WHERE current_status = 'Listed'",
            [],
            |r| r.get(0),
        )?;

        let sold: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM inventory_items WHERE current_status = 'Sold'",
            [],
            |r| r.get(0),
        )?;

        let buyback: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM inventory_items WHERE current_status = 'Buyback'",
            [],
            |r| r.get(0),
        )?;

        let total_retail_value: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(retail_price), 0) FROM inventory_items",
            [],
            |r| r.get(0),
        )?;

        let total_cost: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(cost_price), 0) FROM inventory_items",
            [],
            |r| r.get(0),
        )?;

        let active_auctions: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM auctions WHERE status IN ('Draft', 'Active')",
            [],
            |r| r.get(0),
        )?;

        Ok(DashboardStats {
            total_items,
            in_stock,
            listed,
            sold,
            buyback,
            total_retail_value,
            total_cost,
            active_auctions,
        })
    }
}
