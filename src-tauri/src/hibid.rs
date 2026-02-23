// HiBid Exporter — Generate CSV for Auction Flex / HiBid import

use csv::Writer;
use std::error::Error;

use crate::db::InventoryItemRow;

#[derive(Debug)]
pub struct HiBidLot {
    pub lot_num: String,
    pub lead: String,
    pub description: String,
    pub start_bid: f64,
    pub images: String,
    pub category: String,
}

impl HiBidLot {
    /// Convert an inventory item to a HiBid lot format
    pub fn from_inventory_item(item: &InventoryItemRow) -> Self {
        let lot_num = item.lot_number.as_deref().unwrap_or("0").to_string();

        // Lead: short title (first 50 characters)
        let lead: String = item.raw_title.chars().take(50).collect();

        // Description: full title + retail info
        let condition = item.condition.as_deref().unwrap_or("Unknown");
        let description = format!(
            "{}. Retail Value: ${:.2}. Condition: {}. Quantity: {}.",
            item.raw_title, item.retail_price, condition, item.quantity
        );

        // Images: LotNum-1.jpg, LotNum-2.jpg
        let images = format!("{}-1.jpg,{}-2.jpg", lot_num, lot_num);

        // Category based on extracted data or default
        let category = item
            .category
            .as_deref()
            .unwrap_or("General Merchandise")
            .to_string();

        Self {
            lot_num,
            lead,
            description,
            start_bid: item.min_price,
            images,
            category,
        }
    }
}

/// Export a list of inventory items to a HiBid-compatible CSV file
pub fn export_to_hibid_csv(
    items: &[InventoryItemRow],
    output_path: &str,
) -> Result<usize, Box<dyn Error>> {
    let mut wtr = Writer::from_path(output_path)?;

    // Write header
    wtr.write_record([
        "LotNum",
        "Lead",
        "Description",
        "StartBid",
        "BidIncrement",
        "Images",
        "Category",
    ])?;

    let mut count = 0;
    for item in items {
        let lot = HiBidLot::from_inventory_item(item);

        wtr.write_record([
            &lot.lot_num,
            &lot.lead,
            &lot.description,
            &format!("{:.2}", lot.start_bid),
            "5", // default bid increment
            &lot.images,
            &lot.category,
        ])?;

        count += 1;
    }

    wtr.flush()?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_item() -> InventoryItemRow {
        InventoryItemRow {
            id: "test-id".to_string(),
            manifest_id: "manifest-1".to_string(),
            lot_number: Some("42m".to_string()),
            quantity: 1,
            raw_title: "Samsung 65\" Class 4K UHD Smart TV".to_string(),
            vendor_code: Some("UN65TU7000".to_string()),
            source: Some("Best Buy".to_string()),
            condition: Some("New".to_string()),
            normalized_title: None,
            extracted_brand: Some("Samsung".to_string()),
            extracted_model: None,
            sku_extracted: None,
            category: Some("TVs & Electronics".to_string()),
            retail_price: 549.99,
            cost_price: 77.0,
            min_price: 132.0,
            current_status: "InStock".to_string(),
            auction_id: None,
            listed_at: None,
            sold_at: None,
            created_at: "2026-02-10".to_string(),
            updated_at: "2026-02-10".to_string(),
        }
    }

    #[test]
    fn test_hibid_lot_from_item() {
        let item = mock_item();
        let lot = HiBidLot::from_inventory_item(&item);

        assert_eq!(lot.lot_num, "42m");
        assert!(lot.lead.len() <= 50);
        assert!(lot.description.contains("549.99"));
        assert_eq!(lot.start_bid, 132.0);
        assert_eq!(lot.images, "42m-1.jpg,42m-2.jpg");
        assert_eq!(lot.category, "TVs & Electronics");
    }
}
