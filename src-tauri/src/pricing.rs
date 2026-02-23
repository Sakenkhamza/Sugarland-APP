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
                let min_price =
                    ((cost + retail_price * v.min_price_margin) * 100.0).round() / 100.0;
                (cost, min_price, v.name.clone())
            }
            None => (0.0, 0.0, "Unknown".to_string()),
        }
    }
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
        assert_eq!(min_price, 767.76);
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
