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
    pub read_description_flag: bool,
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
    pub sale_order: Option<i32>,
    pub buybacker_id: Option<String>,
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
             PRAGMA cache_size=-64000;
             PRAGMA ignore_check_constraints=ON;",
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
                status TEXT CHECK(status IN ('Active', 'Completed')) DEFAULT 'Active',
                total_lots INTEGER DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            -- Auction Reports (generated Excel files)
            CREATE TABLE IF NOT EXISTS auction_reports (
                id TEXT PRIMARY KEY,
                auction_id TEXT NOT NULL REFERENCES auctions(id),
                report_type TEXT NOT NULL,
                file_name TEXT NOT NULL,
                file_path TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_auction_reports_auction ON auction_reports(auction_id);

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
                read_description_flag BOOLEAN NOT NULL DEFAULT FALSE,

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
                current_status TEXT CHECK(current_status IN ('InStock', 'Listed', 'Sold', 'Unsold', 'Buyback', 'Scrap')) DEFAULT 'InStock',

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
                SUM(CASE WHEN ar.is_buyback = FALSE THEN COALESCE(ar.high_bid, 0) - COALESCE(i.cost_price, 0) ELSE 0 END) as net_profit
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

        // Simple migration: Add vendor_id to auctions if it doesn't exist
        let _ = self
            .conn
            .execute("ALTER TABLE auctions ADD COLUMN vendor_id TEXT", []);

        // Migration: convert Draft/Cancelled auctions to Active
        let _ = self.conn.execute(
            "UPDATE auctions SET status = 'Active' WHERE status IN ('Draft', 'Cancelled')",
            [],
        );

        // Migration: Add 'Unsold' and 'FloorSale' to inventory_items current_status CHECK constraint
        // Use PRAGMA writable_schema to directly patch the constraint (avoids trigger/view issues)
        let has_floorsale: bool = self
            .conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='inventory_items'",
                [],
                |row| {
                    let sql: String = row.get(0)?;
                    Ok(sql.contains("FloorSale"))
                },
            )
            .unwrap_or(true);

        if !has_floorsale {
            log::info!("Running migration: adding 'Unsold'/'FloorSale' to inventory_items CHECK constraint");
            // First ensure Unsold is present
            let has_unsold: bool = self
                .conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='inventory_items'",
                    [],
                    |row| {
                        let sql: String = row.get(0)?;
                        Ok(sql.contains("Unsold"))
                    },
                )
                .unwrap_or(true);

            if !has_unsold {
                let _ = self.conn.execute_batch("
                    PRAGMA writable_schema = ON;
                    UPDATE sqlite_master
                    SET sql = replace(sql, '''Sold'', ''Buyback''', '''Sold'', ''Unsold'', ''FloorSale'', ''Buyback''')
                    WHERE type = 'table' AND name = 'inventory_items';
                    PRAGMA writable_schema = OFF;
                ");
            } else {
                let _ = self.conn.execute_batch("
                    PRAGMA writable_schema = ON;
                    UPDATE sqlite_master
                    SET sql = replace(sql, '''Unsold'', ''Buyback''', '''Unsold'', ''FloorSale'', ''Buyback''')
                    WHERE type = 'table' AND name = 'inventory_items';
                    PRAGMA writable_schema = OFF;
                ");
            }
            let _ = self.conn.execute_batch("PRAGMA integrity_check;");
        }

        // Migration: Add sale_order and buybacker_id columns to inventory_items
        let _ = self.conn.execute(
            "ALTER TABLE inventory_items ADD COLUMN sale_order INTEGER",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE inventory_items ADD COLUMN buybacker_id TEXT",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE inventory_items ADD COLUMN read_description_flag BOOLEAN NOT NULL DEFAULT FALSE",
            [],
        );
        // Migration: keep per-attempt snapshot data for reliable repeater analytics
        let _ = self.conn.execute(
            "ALTER TABLE auction_results ADD COLUMN item_status TEXT",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE auction_results ADD COLUMN min_price_snapshot REAL",
            [],
        );
        let _ = self.conn.execute(
            "UPDATE auction_results
             SET item_status = CASE
                 WHEN is_buyback = 1 THEN 'Buyback'
                 WHEN COALESCE(high_bid, 0) <= 0 OR trim(COALESCE(winning_bidder, '')) = '' THEN 'Unsold'
                 ELSE 'Sold'
             END
             WHERE item_status IS NULL",
            [],
        );
        let _ = self.conn.execute(
            "UPDATE auction_results
             SET min_price_snapshot = (
                 SELECT i.min_price
                 FROM inventory_items i
                 WHERE i.id = auction_results.item_id
             )
             WHERE min_price_snapshot IS NULL",
            [],
        );
        // Migration: keep historical_sales focused on true sold attempts only
        let _ = self.conn.execute_batch(
            "
            DROP TRIGGER IF EXISTS after_auction_result_insert;
            CREATE TRIGGER IF NOT EXISTS after_auction_result_insert
            AFTER INSERT ON auction_results
            FOR EACH ROW
            WHEN NEW.is_buyback = FALSE AND COALESCE(NEW.item_status, 'Sold') = 'Sold'
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
            ",
        );
        // Migration: refresh v_auction_pnl to use persisted auction_results item_status
        let _ = self.conn.execute_batch(
            "
            DROP VIEW IF EXISTS v_auction_pnl;
            CREATE VIEW IF NOT EXISTS v_auction_pnl AS
            SELECT
                a.id as auction_id,
                a.name as auction_name,
                a.start_date,
                a.end_date,
                COUNT(DISTINCT i.id) as total_items,
                SUM(
                    CASE
                        WHEN COALESCE(
                            ar.item_status,
                            CASE
                                WHEN ar.is_buyback = TRUE THEN 'Buyback'
                                WHEN COALESCE(ar.high_bid, 0) > 0 THEN 'Sold'
                                ELSE 'Unsold'
                            END
                        ) = 'Sold' THEN 1
                        ELSE 0
                    END
                ) as sold_items,
                SUM(
                    CASE
                        WHEN COALESCE(
                            ar.item_status,
                            CASE
                                WHEN ar.is_buyback = TRUE THEN 'Buyback'
                                WHEN COALESCE(ar.high_bid, 0) > 0 THEN 'Sold'
                                ELSE 'Unsold'
                            END
                        ) = 'Buyback' THEN 1
                        ELSE 0
                    END
                ) as buyback_items,
                SUM(
                    CASE
                        WHEN COALESCE(
                            ar.item_status,
                            CASE
                                WHEN ar.is_buyback = TRUE THEN 'Buyback'
                                WHEN COALESCE(ar.high_bid, 0) > 0 THEN 'Sold'
                                ELSE 'Unsold'
                            END
                        ) = 'Sold' THEN ar.high_bid
                        ELSE 0
                    END
                ) as total_revenue,
                SUM(
                    CASE
                        WHEN COALESCE(
                            ar.item_status,
                            CASE
                                WHEN ar.is_buyback = TRUE THEN 'Buyback'
                                WHEN COALESCE(ar.high_bid, 0) > 0 THEN 'Sold'
                                ELSE 'Unsold'
                            END
                        ) = 'Sold' THEN i.cost_price
                        ELSE 0
                    END
                ) as total_cost,
                SUM(
                    CASE
                        WHEN COALESCE(
                            ar.item_status,
                            CASE
                                WHEN ar.is_buyback = TRUE THEN 'Buyback'
                                WHEN COALESCE(ar.high_bid, 0) > 0 THEN 'Sold'
                                ELSE 'Unsold'
                            END
                        ) = 'Sold' THEN ar.commission_amount
                        ELSE 0
                    END
                ) as total_commission,
                SUM(
                    CASE
                        WHEN COALESCE(
                            ar.item_status,
                            CASE
                                WHEN ar.is_buyback = TRUE THEN 'Buyback'
                                WHEN COALESCE(ar.high_bid, 0) > 0 THEN 'Sold'
                                ELSE 'Unsold'
                            END
                        ) = 'Sold' THEN COALESCE(ar.high_bid, 0) - COALESCE(i.cost_price, 0)
                        ELSE 0
                    END
                ) as net_profit
            FROM auctions a
            LEFT JOIN inventory_items i ON i.auction_id = a.id
            LEFT JOIN auction_results ar ON ar.auction_id = a.id AND ar.item_id = i.id
            GROUP BY a.id;
            ",
        );

        // ============================================================
        // New tables for ТЗ: condition_types, source_types, pricing_rules, buybackers
        // ============================================================
        self.conn.execute_batch("
            -- Condition types reference table
            CREATE TABLE IF NOT EXISTS condition_types (
                id TEXT PRIMARY KEY,
                label TEXT NOT NULL UNIQUE,
                category TEXT NOT NULL CHECK(category IN ('New', 'Used', 'Renewed', 'Broken'))
            );

            -- Source types reference table
            CREATE TABLE IF NOT EXISTS source_types (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
            );

            -- Pricing rules matrix (condition-based)
            CREATE TABLE IF NOT EXISTS pricing_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                condition_category TEXT NOT NULL CHECK(condition_category IN ('New', 'Used', 'Renewed', 'Broken')),
                level INTEGER NOT NULL CHECK(level BETWEEN 1 AND 3),
                multiplier REAL NOT NULL,
                label TEXT,
                UNIQUE(condition_category, level)
            );

            -- Buy-backer registry
            DROP TABLE IF EXISTS buybackers;
            CREATE TABLE IF NOT EXISTS buybackers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                is_active BOOLEAN DEFAULT TRUE,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_inventory_sale_order ON inventory_items(sale_order);
            CREATE INDEX IF NOT EXISTS idx_inventory_buybacker ON inventory_items(buybacker_id);
        ")?;

        // Seed condition types
        self.conn.execute_batch(
            "
            INSERT OR IGNORE INTO condition_types (id, label, category) VALUES
                ('new_canceled', 'New - Canceled delivery', 'New');
            INSERT OR IGNORE INTO condition_types (id, label, category) VALUES
                ('new_cosmetic', 'New - Cosmetic flaws', 'New');
            INSERT OR IGNORE INTO condition_types (id, label, category) VALUES
                ('new_packaging', 'New - Packaging flawed', 'New');
            INSERT OR IGNORE INTO condition_types (id, label, category) VALUES
                ('new_sealed', 'New - Factory sealed', 'New');
            INSERT OR IGNORE INTO condition_types (id, label, category) VALUES
                ('new_no_orig_pkg', 'New - Not in original packaging', 'New');
            INSERT OR IGNORE INTO condition_types (id, label, category) VALUES
                ('new_open_box', 'New - Open box', 'New');
            INSERT OR IGNORE INTO condition_types (id, label, category) VALUES
                ('renewed', 'Renewed', 'Renewed');
            INSERT OR IGNORE INTO condition_types (id, label, category) VALUES
                ('used_acceptable', 'Used - Acceptable', 'Used');
            INSERT OR IGNORE INTO condition_types (id, label, category) VALUES
                ('used_good', 'Used - Good', 'Used');
            INSERT OR IGNORE INTO condition_types (id, label, category) VALUES
                ('used_like_new', 'Used - Like new and open box', 'Used');
            INSERT OR IGNORE INTO condition_types (id, label, category) VALUES
                ('used_very_good', 'Used - Very good', 'Used');
            INSERT OR IGNORE INTO condition_types (id, label, category) VALUES
                ('broken', 'Broken', 'Broken');
        ",
        )?;

        // Seed source types
        self.conn.execute_batch(
            "
            INSERT OR IGNORE INTO source_types (id, name) VALUES ('amazon', 'Amazon Bstock');
            INSERT OR IGNORE INTO source_types (id, name) VALUES ('bestbuy', 'Best Buy');
            INSERT OR IGNORE INTO source_types (id, name) VALUES ('wayfair', 'Wayfair');
            INSERT OR IGNORE INTO source_types (id, name) VALUES ('mech', 'Mech/PDX7');
            INSERT OR IGNORE INTO source_types (id, name) VALUES ('target', 'Target');
            INSERT OR IGNORE INTO source_types (id, name) VALUES ('costco', 'Costco');
            INSERT OR IGNORE INTO source_types (id, name) VALUES ('homedepot', 'Home Depot');
            INSERT OR IGNORE INTO source_types (id, name) VALUES ('lowes', 'Lowes');
            INSERT OR IGNORE INTO source_types (id, name) VALUES ('walmart', 'Walmart');
            INSERT OR IGNORE INTO source_types (id, name) VALUES ('unknown', 'Unknown');
        ",
        )?;

        // Seed pricing rules matrix
        // New: L1=Cost(1.0), L2=Cost+5%(1.05), L3=Cost+10%(1.10)
        // Used: L1=Cost(1.0), L2=Cost+3%(1.03), L3=Cost+6%(1.06)
        // Renewed: same as Used
        // Broken: L1=0.5*Cost, L2=0.75*Cost, L3=1.02*Cost (max 2% recovery)
        self.conn.execute_batch("
            INSERT OR IGNORE INTO pricing_rules (condition_category, level, multiplier, label) VALUES
                ('New', 1, 1.00, 'Cost (Base)');
            INSERT OR IGNORE INTO pricing_rules (condition_category, level, multiplier, label) VALUES
                ('New', 2, 1.05, 'Cost + 5%');
            INSERT OR IGNORE INTO pricing_rules (condition_category, level, multiplier, label) VALUES
                ('New', 3, 1.10, 'Cost + 10%');
            INSERT OR IGNORE INTO pricing_rules (condition_category, level, multiplier, label) VALUES
                ('Used', 1, 1.00, 'Cost (Base)');
            INSERT OR IGNORE INTO pricing_rules (condition_category, level, multiplier, label) VALUES
                ('Used', 2, 1.03, 'Cost + 3%');
            INSERT OR IGNORE INTO pricing_rules (condition_category, level, multiplier, label) VALUES
                ('Used', 3, 1.06, 'Cost + 6%');
            INSERT OR IGNORE INTO pricing_rules (condition_category, level, multiplier, label) VALUES
                ('Renewed', 1, 1.00, 'Cost (Base)');
            INSERT OR IGNORE INTO pricing_rules (condition_category, level, multiplier, label) VALUES
                ('Renewed', 2, 1.03, 'Cost + 3%');
            INSERT OR IGNORE INTO pricing_rules (condition_category, level, multiplier, label) VALUES
                ('Renewed', 3, 1.06, 'Cost + 6%');
            INSERT OR IGNORE INTO pricing_rules (condition_category, level, multiplier, label) VALUES
                ('Broken', 1, 0.50, '50% of Cost');
            INSERT OR IGNORE INTO pricing_rules (condition_category, level, multiplier, label) VALUES
                ('Broken', 2, 0.75, '75% of Cost');
            INSERT OR IGNORE INTO pricing_rules (condition_category, level, multiplier, label) VALUES
                ('Broken', 3, 1.02, 'Max 2% recovery');
        ")?;

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
                ('app_version', '0.3.0', 'Current application version', 'system');
            INSERT OR IGNORE INTO settings (key, value, description, category) VALUES
                ('db_version', '3', 'Database schema version', 'system');
            ",
        )?;

        // One-time migration: legacy HiBid imports stored cents instead of dollars.
        // Apply only once and persist a migration flag in settings.
        let hibid_cents_fix_applied: bool = self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'hibid_cents_fix_applied'",
                [],
                |row| row.get::<_, String>(0),
            )
            .map(|value| value == "1")
            .unwrap_or(false);

        if !hibid_cents_fix_applied {
            log::info!("Running migration: normalize legacy HiBid cents values to dollars");
            let _ = self.conn.execute(
                "UPDATE auction_results
                 SET high_bid = high_bid / 100.0
                 WHERE high_bid >= 1000",
                [],
            );
            let _ = self.conn.execute(
                "UPDATE auction_results
                 SET max_bid = max_bid / 100.0
                 WHERE max_bid IS NOT NULL
                   AND max_bid >= 1000",
                [],
            );
            let _ = self.conn.execute(
                "UPDATE auction_results
                 SET commission_amount = CASE
                   WHEN COALESCE(
                     item_status,
                     CASE
                       WHEN is_buyback = 1 THEN 'Buyback'
                       WHEN COALESCE(high_bid, 0) > 0 THEN 'Sold'
                       ELSE 'Unsold'
                     END
                   ) = 'Sold'
                     THEN ROUND(COALESCE(high_bid, 0) * COALESCE(commission_rate, 0.15), 4)
                   ELSE 0
                 END",
                [],
            );
            let _ = self.conn.execute(
                "UPDATE auction_results
                 SET net_profit = CASE
                   WHEN COALESCE(
                     item_status,
                     CASE
                       WHEN is_buyback = 1 THEN 'Buyback'
                       WHEN COALESCE(high_bid, 0) > 0 THEN 'Sold'
                       ELSE 'Unsold'
                     END
                   ) = 'Sold'
                      THEN ROUND(
                        COALESCE(high_bid, 0)
                        - COALESCE(
                          (SELECT i.cost_price FROM inventory_items i WHERE i.id = auction_results.item_id),
                          0
                        ),
                        4
                      )
                    ELSE 0
                  END",
                [],
            );
            let _ = self.conn.execute(
                "UPDATE historical_sales
                 SET sale_price = COALESCE(
                   (SELECT ar.high_bid FROM auction_results ar WHERE ar.id = historical_sales.id),
                   sale_price
                 )
                 WHERE platform = 'HiBid'",
                [],
            );
            let _ = self.conn.execute(
                "INSERT INTO settings (key, value, description, category)
                 VALUES ('hibid_cents_fix_applied', '1', 'One-time migration for HiBid cents-to-dollars fix', 'system')
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   description = excluded.description,
                   category = excluded.category",
                [],
            );
        }

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
                    raw_title, vendor_code, source, condition, read_description_flag,
                    normalized_title, extracted_brand, extracted_model, sku_extracted, category,
                    retail_price, cost_price, min_price,
                    current_status, auction_id, listed_at, sold_at,
                    sale_order, buybacker_id,
                    created_at, updated_at
             FROM inventory_items WHERE 1=1",
        );

        if let Some(s) = status {
            query.push_str(&format!(" AND current_status = '{}'", s));
        } else {
            query.push_str(" AND current_status != 'Sold'");
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
                    read_description_flag: row.get::<_, Option<bool>>(8)?.unwrap_or(false),
                    normalized_title: row.get(9)?,
                    extracted_brand: row.get(10)?,
                    extracted_model: row.get(11)?,
                    sku_extracted: row.get(12)?,
                    category: row.get(13)?,
                    retail_price: row.get(14)?,
                    cost_price: row.get(15)?,
                    min_price: row.get(16)?,
                    current_status: row.get(17)?,
                    auction_id: row.get(18)?,
                    listed_at: row.get(19)?,
                    sold_at: row.get(20)?,
                    sale_order: row.get(21)?,
                    buybacker_id: row.get(22)?,
                    created_at: row.get(23)?,
                    updated_at: row.get(24)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(items)
    }

    pub fn get_dashboard_stats(&self) -> Result<DashboardStats> {
        let total_items: i64 =
            self.conn
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
