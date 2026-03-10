// Pricing Engine — Vendor-based cost calculation

use rusqlite::{Connection, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Vendor {
    pub id: String,
    pub name: String,
    pub cost_coefficient: f64,
    pub min_price_margin: f64,
    pub is_active: bool,
}

pub struct PricingEngine {
    vendors: Vec<Vendor>,
}

impl PricingEngine {
    /// Create a new PricingEngine, loading vendors from the database
    pub fn new(conn: &Connection) -> Result<Self> {
        let vendors = Self::load_vendors(conn)?;
        Ok(Self { vendors })
    }

    /// Load all active vendors from the database
    pub fn load_vendors(conn: &Connection) -> Result<Vec<Vendor>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, cost_coefficient, min_price_margin, is_active
             FROM vendors WHERE is_active = TRUE",
        )?;

        let vendors = stmt
            .query_map([], |row| {
                Ok(Vendor {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    cost_coefficient: row.get(2)?,
                    min_price_margin: row.get(3)?,
                    is_active: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(vendors)
    }

    /// Calculate cost and minimum price for a given retail price and source
    ///
    /// Returns (cost_price, min_price, vendor_name)
    ///
    /// Formula:
    ///   cost = retail_price × vendor.cost_coefficient
    ///   min_price = cost + (retail_price × vendor.min_price_margin)
    pub fn calculate_cost(&self, retail_price: f64, source: &str) -> (f64, f64, String) {
        // Find matching vendor by source name
        let vendor = self
            .vendors
            .iter()
            .find(|v| source.to_lowercase().contains(&v.name.to_lowercase()))
            .or_else(|| {
                // Fallback to Amazon Bstock as default
                self.vendors.iter().find(|v| v.name == "Amazon Bstock")
            });

        match vendor {
            Some(v) => {
                let cost = (retail_price * v.cost_coefficient * 100.0).round() / 100.0;
                let min_price = (cost + retail_price * v.min_price_margin).ceil();
                (cost, min_price, v.name.clone())
            }
            None => (0.0, 0.0, "Unknown".to_string()),
        }
    }

    /// Calculate condition-based minimum price using the pricing rules matrix
    ///
    /// Returns the min price for a given cost, condition, and pricing level (1-3)
    pub fn calculate_condition_price(cost: f64, condition: &str, level: u8, conn: &Connection) -> f64 {
        let category = Self::condition_to_category(condition);
        let lvl = level.max(1).min(3) as i32;

        let multiplier: f64 = conn.query_row(
            "SELECT multiplier FROM pricing_rules WHERE condition_category = ?1 AND level = ?2",
            rusqlite::params![category, lvl],
            |row| row.get(0),
        ).unwrap_or(1.0);

        (cost * multiplier * 100.0).round() / 100.0
    }

    /// Map a condition label to its pricing category (New, Used, Renewed, Broken)
    pub fn condition_to_category(condition: &str) -> &str {
        let lower = condition.to_lowercase();
        if lower.starts_with("new") {
            "New"
        } else if lower.starts_with("used") {
            "Used"
        } else if lower.contains("renewed") || lower.contains("refurbished") {
            "Renewed"
        } else if lower.contains("broken") || lower.contains("damaged") || lower.contains("for parts") {
            "Broken"
        } else {
            "New" // Default to New for unclassified items
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PricingRule {
    pub id: i32,
    pub condition_category: String,
    pub level: i32,
    pub multiplier: f64,
    pub label: Option<String>,
}

/// Load all pricing rules from the database
pub fn get_pricing_rules(conn: &Connection) -> Result<Vec<PricingRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, condition_category, level, multiplier, label
         FROM pricing_rules ORDER BY condition_category, level"
    )?;

    let rules = stmt.query_map([], |row| {
        Ok(PricingRule {
            id: row.get(0)?,
            condition_category: row.get(1)?,
            level: row.get(2)?,
            multiplier: row.get(3)?,
            label: row.get(4)?,
        })
    })?.collect::<Result<Vec<_>>>()?;

    Ok(rules)
}

/// Update a pricing rule multiplier
pub fn update_pricing_rule(conn: &Connection, condition_category: &str, level: i32, multiplier: f64) -> Result<()> {
    conn.execute(
        "UPDATE pricing_rules SET multiplier = ?1 WHERE condition_category = ?2 AND level = ?3",
        rusqlite::params![multiplier, condition_category, level],
    )?;
    Ok(())
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> PricingEngine {
        PricingEngine {
            vendors: vec![
                Vendor {
                    id: "bestbuy".to_string(),
                    name: "Best Buy".to_string(),
                    cost_coefficient: 0.14,
                    min_price_margin: 0.10,
                    is_active: true,
                },
                Vendor {
                    id: "wayfair".to_string(),
                    name: "Wayfair".to_string(),
                    cost_coefficient: 0.07,
                    min_price_margin: 0.10,
                    is_active: true,
                },
                Vendor {
                    id: "mech".to_string(),
                    name: "Mech/PDX7".to_string(),
                    cost_coefficient: 0.20,
                    min_price_margin: 0.10,
                    is_active: true,
                },
                Vendor {
                    id: "amazon".to_string(),
                    name: "Amazon Bstock".to_string(),
                    cost_coefficient: 0.20,
                    min_price_margin: 0.10,
                    is_active: true,
                },
            ],
        }
    }

    #[test]
    fn test_best_buy_pricing() {
        let engine = make_engine();
        let (cost, min_price, vendor) = engine.calculate_cost(3199.0, "Best Buy");

        assert_eq!(vendor, "Best Buy");
        assert_eq!(cost, 447.86);
        assert_eq!(min_price, 768.0);
    }

    #[test]
    fn test_wayfair_pricing() {
        let engine = make_engine();
        let (cost, min_price, vendor) = engine.calculate_cost(1000.0, "Wayfair");

        assert_eq!(vendor, "Wayfair");
        assert_eq!(cost, 70.0);
        assert_eq!(min_price, 170.0);
    }

    #[test]
    fn test_unknown_source_fallback() {
        let engine = make_engine();
        let (_cost, _min_price, vendor) = engine.calculate_cost(500.0, "Unknown Vendor");

        // Should fall back to Amazon Bstock
        assert_eq!(vendor, "Amazon Bstock");
    }
}
