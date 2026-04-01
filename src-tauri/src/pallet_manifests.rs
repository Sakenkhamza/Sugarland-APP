use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Formula, Workbook, Worksheet};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize)]
pub struct PalletManifestExportResult {
    pub file_path: String,
    pub items_count: usize,
    pub pallets_count: usize,
}

#[derive(Debug, Deserialize)]
struct RawPalletManifestRow {
    #[serde(rename = "Category", default)]
    category: String,
    #[serde(rename = "Subcategory", default)]
    subcategory: String,
    #[serde(rename = "ASIN", default)]
    asin: String,
    #[serde(rename = "Item Description", default)]
    item_description: String,
    #[serde(rename = "Qty", default)]
    qty: String,
    #[serde(rename = "Unit Retail", default)]
    unit_retail: String,
    #[serde(rename = "Ext. Retail", default)]
    ext_retail: String,
    #[serde(rename = "Product Class", default)]
    product_class: String,
    #[serde(rename = "GL Description", default)]
    gl_description: String,
    #[serde(rename = "Seller Category", default)]
    seller_category: String,
    #[serde(rename = "EAN", default)]
    ean: String,
    #[serde(rename = "LPN", default)]
    lpn: String,
    #[serde(rename = "UPC", default)]
    upc: String,
    #[serde(rename = "Brand", default)]
    brand: String,
    #[serde(rename = "Condition", default)]
    condition: String,
    #[serde(rename = "Pallet ID", default)]
    pallet_id: String,
    #[serde(rename = "Lot ID", default)]
    lot_id: String,
}

#[derive(Debug, Clone)]
struct PalletManifestRow {
    category: String,
    subcategory: String,
    asin: String,
    item_description: String,
    qty: f64,
    unit_retail_text: String,
    ext_retail: f64,
    brand: String,
    pallet_id: String,
}

#[derive(Debug, Clone)]
struct PalletGroup {
    pallet_id: String,
    rows: Vec<PalletManifestRow>,
    asin_count: usize,
    qty_total: f64,
    ext_total: f64,
}

#[derive(Debug, Clone)]
struct BrandSummary {
    brand: String,
    qty_total: f64,
    ext_total: f64,
}

fn parse_number(value: &str) -> f64 {
    value
        .replace('$', "")
        .replace(',', "")
        .trim()
        .parse::<f64>()
        .unwrap_or(0.0)
}

fn formula_result(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn build_raw_sheet_name(file_path: &str) -> String {
    let stem = Path::new(file_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Manifest");
    stem.chars().take(31).collect()
}

fn read_raw_lines(file_path: &str) -> Result<Vec<String>, String> {
    let file = File::open(file_path)
        .map_err(|e| format!("Failed to open CSV file for raw import: {e}"))?;
    let reader = BufReader::new(file);
    reader
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read CSV lines: {e}"))
}

fn parse_pallet_manifest_csv(file_path: &str) -> Result<Vec<PalletManifestRow>, String> {
    let file =
        File::open(file_path).map_err(|e| format!("Failed to open pallet manifest CSV: {e}"))?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(file);

    let mut rows = Vec::new();

    for (index, result) in reader.deserialize::<RawPalletManifestRow>().enumerate() {
        let raw = result.map_err(|e| format!("Failed to parse CSV row {}: {e}", index + 2))?;
        let _ = (
            &raw.product_class,
            &raw.gl_description,
            &raw.seller_category,
            &raw.ean,
            &raw.lpn,
            &raw.upc,
            &raw.condition,
            &raw.lot_id,
        );

        rows.push(PalletManifestRow {
            category: raw.category,
            subcategory: raw.subcategory,
            asin: raw.asin,
            item_description: raw.item_description,
            qty: parse_number(&raw.qty),
            unit_retail_text: format!("{:.2}", parse_number(&raw.unit_retail)),
            ext_retail: parse_number(&raw.ext_retail),
            brand: raw.brand,
            pallet_id: raw.pallet_id,
        });
    }

    if rows.is_empty() {
        return Err("The selected CSV file doesn't contain any manifest rows".to_string());
    }

    Ok(rows)
}

fn build_pallet_groups(rows: &[PalletManifestRow]) -> Vec<PalletGroup> {
    let mut grouped: BTreeMap<String, Vec<PalletManifestRow>> = BTreeMap::new();

    for row in rows {
        grouped
            .entry(row.pallet_id.clone())
            .or_default()
            .push(row.clone());
    }

    grouped
        .into_iter()
        .map(|(pallet_id, rows)| {
            let asin_count = rows.len();
            let qty_total = rows.iter().map(|row| row.qty).sum::<f64>();
            let ext_total = rows.iter().map(|row| row.ext_retail).sum::<f64>();

            PalletGroup {
                pallet_id,
                rows,
                asin_count,
                qty_total,
                ext_total,
            }
        })
        .collect()
}

fn build_brand_summaries(rows: &[PalletManifestRow]) -> Vec<BrandSummary> {
    let mut grouped: HashMap<String, (f64, f64)> = HashMap::new();

    for row in rows {
        let brand_key = if row.brand.trim().is_empty() {
            "(blank)".to_string()
        } else {
            row.brand.trim().to_string()
        };

        let entry = grouped.entry(brand_key).or_insert((0.0, 0.0));
        entry.0 += row.qty;
        entry.1 += row.ext_retail;
    }

    let mut result = grouped
        .into_iter()
        .map(|(brand, (qty_total, ext_total))| BrandSummary {
            brand,
            qty_total,
            ext_total,
        })
        .collect::<Vec<_>>();

    result.sort_by(|left, right| {
        right
            .ext_total
            .partial_cmp(&left.ext_total)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.brand.to_lowercase().cmp(&right.brand.to_lowercase()))
    });

    result
}

fn finalize_output_path(save_path: &str) -> String {
    let path = Path::new(save_path);
    match path.extension().and_then(|value| value.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("xlsx") => save_path.to_string(),
        _ => format!("{save_path}.xlsx"),
    }
}

fn ensure_parent_dir(path: &str) -> Result<(), String> {
    let output_path = PathBuf::from(path);
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create output directory: {e}"))?;
        }
    }
    Ok(())
}

fn write_raw_sheet(workbook: &mut Workbook, sheet_name: &str, raw_lines: &[String]) -> Result<(), String> {
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name(sheet_name)
        .map_err(|e| format!("Failed to set raw sheet name: {e}"))?;
    worksheet.set_hidden(true);

    for (row_index, line) in raw_lines.iter().enumerate() {
        worksheet
            .write_string(row_index as u32, 0, line)
            .map_err(|e| format!("Failed to write raw CSV row: {e}"))?;
    }

    Ok(())
}

fn write_list_sheet(workbook: &mut Workbook, groups: &[PalletGroup]) -> Result<(), String> {
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name("List")
        .map_err(|e| format!("Failed to set List sheet name: {e}"))?;
    worksheet.set_zoom(60).set_active(true).set_selected(true);

    let header_format = Format::new().set_bold().set_border(FormatBorder::Thin);
    let text_cell_format = Format::new().set_border(FormatBorder::Thin);
    let number_cell_format = Format::new()
        .set_border(FormatBorder::Thin)
        .set_num_format("#,##0");

    let headers = [
        "Pallet ID",
        "Count of ASIN",
        "Sum of Ext, Retail",
        "Pallet price",
    ];
    let widths = [30.109375, 12.44140625, 15.33203125, 10.6640625];

    for (index, width) in widths.iter().enumerate() {
        worksheet
            .set_column_width(index as u16, *width)
            .map_err(|e| format!("Failed to set List column width: {e}"))?;
    }

    for (col, header) in headers.iter().enumerate() {
        worksheet
            .write_string_with_format(0, col as u16, *header, &header_format)
            .map_err(|e| format!("Failed to write List header: {e}"))?;
    }

    for (index, group) in groups.iter().enumerate() {
        let row = index as u32 + 1;
        worksheet
            .write_string_with_format(row, 0, &group.pallet_id, &text_cell_format)
            .map_err(|e| format!("Failed to write List pallet id: {e}"))?;
        worksheet
            .write_number_with_format(row, 1, group.asin_count as f64, &number_cell_format)
            .map_err(|e| format!("Failed to write List ASIN count: {e}"))?;
        worksheet
            .write_number_with_format(row, 2, group.ext_total, &number_cell_format)
            .map_err(|e| format!("Failed to write List retail sum: {e}"))?;
        worksheet
            .write_number_with_format(row, 3, group.ext_total * 0.2, &number_cell_format)
            .map_err(|e| format!("Failed to write List pallet price: {e}"))?;
    }

    worksheet
        .autofilter(0, 0, 0, 3)
        .map_err(|e| format!("Failed to set List autofilter: {e}"))?;

    Ok(())
}

fn write_sheet3(workbook: &mut Workbook, brands: &[BrandSummary]) -> Result<(), String> {
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name("Sheet3")
        .map_err(|e| format!("Failed to set Sheet3 name: {e}"))?;
    worksheet.set_hidden(true);

    worksheet
        .set_column_width(0, 35.77734375)
        .map_err(|e| format!("Failed to set Sheet3 width: {e}"))?;
    worksheet
        .set_column_width(1, 10.44140625)
        .map_err(|e| format!("Failed to set Sheet3 width: {e}"))?;
    worksheet
        .set_column_width(2, 15.77734375)
        .map_err(|e| format!("Failed to set Sheet3 width: {e}"))?;

    let header_format = Format::new().set_bold();

    worksheet
        .write_string_with_format(3, 1, "Sum of Qty", &header_format)
        .map_err(|e| format!("Failed to write Sheet3 header: {e}"))?;
    worksheet
        .write_string_with_format(3, 2, "Sum of Ext, Retail", &header_format)
        .map_err(|e| format!("Failed to write Sheet3 header: {e}"))?;

    for (index, brand) in brands.iter().enumerate() {
        let row = index as u32 + 4;
        worksheet
            .write_string(row, 0, &brand.brand)
            .map_err(|e| format!("Failed to write Sheet3 brand: {e}"))?;
        worksheet
            .write_number(row, 1, brand.qty_total)
            .map_err(|e| format!("Failed to write Sheet3 qty: {e}"))?;
        worksheet
            .write_number(row, 2, brand.ext_total)
            .map_err(|e| format!("Failed to write Sheet3 retail: {e}"))?;
    }

    Ok(())
}

fn write_sheet2(workbook: &mut Workbook, groups: &[PalletGroup]) -> Result<(), String> {
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name("Sheet2")
        .map_err(|e| format!("Failed to set Sheet2 name: {e}"))?;
    worksheet.set_hidden(true);

    worksheet
        .set_column_width(0, 28.88671875)
        .map_err(|e| format!("Failed to set Sheet2 width: {e}"))?;
    worksheet
        .set_column_width(1, 12.88671875)
        .map_err(|e| format!("Failed to set Sheet2 width: {e}"))?;
    worksheet
        .set_column_width(2, 15.77734375)
        .map_err(|e| format!("Failed to set Sheet2 width: {e}"))?;

    let title_format = Format::new().set_bold();
    let header_format = Format::new().set_bold();

    worksheet
        .write_string_with_format(2, 1, "Values", &title_format)
        .map_err(|e| format!("Failed to write Sheet2 title: {e}"))?;
    worksheet
        .write_string_with_format(3, 0, "Pallet ID", &header_format)
        .map_err(|e| format!("Failed to write Sheet2 header: {e}"))?;
    worksheet
        .write_string_with_format(3, 1, "Count of ASIN", &header_format)
        .map_err(|e| format!("Failed to write Sheet2 header: {e}"))?;
    worksheet
        .write_string_with_format(3, 2, "Sum of Ext, Retail", &header_format)
        .map_err(|e| format!("Failed to write Sheet2 header: {e}"))?;

    for (index, group) in groups.iter().enumerate() {
        let row = index as u32 + 4;
        worksheet
            .write_string(row, 0, &group.pallet_id)
            .map_err(|e| format!("Failed to write Sheet2 pallet id: {e}"))?;
        worksheet
            .write_number(row, 1, group.asin_count as f64)
            .map_err(|e| format!("Failed to write Sheet2 count: {e}"))?;
        worksheet
            .write_number(row, 2, group.ext_total)
            .map_err(|e| format!("Failed to write Sheet2 retail: {e}"))?;
    }

    Ok(())
}

fn write_sheet4(workbook: &mut Workbook, groups: &[PalletGroup]) -> Result<(), String> {
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name("Sheet4")
        .map_err(|e| format!("Failed to set Sheet4 name: {e}"))?;
    worksheet.set_hidden(true);

    worksheet
        .set_column_width(0, 4.6640625)
        .map_err(|e| format!("Failed to set Sheet4 width: {e}"))?;
    worksheet
        .set_column_width(1, 28.88671875)
        .map_err(|e| format!("Failed to set Sheet4 width: {e}"))?;
    worksheet
        .set_column_width(2, 12.44140625)
        .map_err(|e| format!("Failed to set Sheet4 width: {e}"))?;
    worksheet
        .set_column_width(3, 15.33203125)
        .map_err(|e| format!("Failed to set Sheet4 width: {e}"))?;

    let header_format = Format::new().set_bold().set_border(FormatBorder::Thin);
    let cell_format = Format::new().set_border(FormatBorder::Thin);

    worksheet
        .write_string_with_format(1, 1, "Pallet ID", &header_format)
        .map_err(|e| format!("Failed to write Sheet4 header: {e}"))?;
    worksheet
        .write_string_with_format(1, 2, "Count of ASIN", &header_format)
        .map_err(|e| format!("Failed to write Sheet4 header: {e}"))?;
    worksheet
        .write_string_with_format(1, 3, "Sum of Ext, Retail", &header_format)
        .map_err(|e| format!("Failed to write Sheet4 header: {e}"))?;

    for (index, group) in groups.iter().enumerate() {
        let row = index as u32 + 2;
        worksheet
            .write_string_with_format(row, 1, &group.pallet_id, &cell_format)
            .map_err(|e| format!("Failed to write Sheet4 pallet id: {e}"))?;
        worksheet
            .write_number_with_format(row, 2, group.asin_count as f64, &cell_format)
            .map_err(|e| format!("Failed to write Sheet4 count: {e}"))?;
        worksheet
            .write_number_with_format(row, 3, group.ext_total, &cell_format)
            .map_err(|e| format!("Failed to write Sheet4 retail: {e}"))?;
    }

    Ok(())
}

fn write_pallets_sheet(workbook: &mut Workbook, groups: &[PalletGroup]) -> Result<(), String> {
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name("Pallets")
        .map_err(|e| format!("Failed to set Pallets sheet name: {e}"))?;
    worksheet.set_zoom(60).set_landscape();

    worksheet
        .set_column_width(0, 25.77734375)
        .map_err(|e| format!("Failed to set Pallets width: {e}"))?;
    worksheet
        .set_column_width(1, 30.0)
        .map_err(|e| format!("Failed to set Pallets width: {e}"))?;
    worksheet
        .set_column_width(2, 14.0)
        .map_err(|e| format!("Failed to set Pallets width: {e}"))?;
    worksheet
        .set_column_width(3, 80.88671875)
        .map_err(|e| format!("Failed to set Pallets width: {e}"))?;
    worksheet
        .set_column_width(4, 3.88671875)
        .map_err(|e| format!("Failed to set Pallets width: {e}"))?;
    worksheet
        .set_column_width(5, 9.5546875)
        .map_err(|e| format!("Failed to set Pallets width: {e}"))?;
    worksheet
        .set_column_width(6, 9.21875)
        .map_err(|e| format!("Failed to set Pallets width: {e}"))?;
    worksheet
        .set_column_width(7, 28.88671875)
        .map_err(|e| format!("Failed to set Pallets width: {e}"))?;
    worksheet
        .set_column_hidden(0)
        .map_err(|e| format!("Failed to hide Pallets column A: {e}"))?;
    worksheet
        .set_column_hidden(1)
        .map_err(|e| format!("Failed to hide Pallets column B: {e}"))?;

    let hidden_header_format = Format::new().set_bold();
    let hidden_value_format = Format::new();
    let header_format = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_background_color(Color::RGB(0xD9E1F2))
        .set_align(FormatAlign::Center);
    let header_number_format = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_background_color(Color::RGB(0xD9E1F2))
        .set_align(FormatAlign::Center)
        .set_num_format("#,##0.00");
    let data_text_format = Format::new().set_border(FormatBorder::Thin);
    let data_number_format = Format::new()
        .set_border(FormatBorder::Thin)
        .set_num_format("#,##0.00");
    let summary_hidden_format = Format::new().set_border(FormatBorder::Thin);
    let summary_text_format = Format::new()
        .set_bold()
        .set_border_left(FormatBorder::Thin)
        .set_border_right(FormatBorder::Thin)
        .set_border_top(FormatBorder::Thin)
        .set_background_color(Color::RGB(0xD9E1F2))
        .set_align(FormatAlign::Center);
    let summary_price_format = Format::new()
        .set_bold()
        .set_font_size(14)
        .set_border_left(FormatBorder::Thin)
        .set_border_right(FormatBorder::Thin)
        .set_border_top(FormatBorder::Thin)
        .set_background_color(Color::RGB(0xD9E1F2))
        .set_align(FormatAlign::Left)
        .set_num_format("0");
    let summary_number_format = Format::new()
        .set_bold()
        .set_border_left(FormatBorder::Thin)
        .set_border_right(FormatBorder::Thin)
        .set_border_top(FormatBorder::Thin)
        .set_background_color(Color::RGB(0xD9E1F2))
        .set_align(FormatAlign::Center)
        .set_num_format("#,##0");
    let summary_blank_format = Format::new()
        .set_border_left(FormatBorder::Thin)
        .set_border_right(FormatBorder::Thin)
        .set_border_top(FormatBorder::Thin)
        .set_align(FormatAlign::Center);

    let headers = [
        "Category",
        "Subcategory",
        "ASIN",
        "Item Description",
        "Qty",
        "Unit Retail",
        "Ext, Retail",
        "Pallet ID",
    ];

    worksheet
        .write_string_with_format(0, 0, headers[0], &hidden_header_format)
        .map_err(|e| format!("Failed to write Pallets header: {e}"))?;
    worksheet
        .write_string_with_format(0, 1, headers[1], &hidden_header_format)
        .map_err(|e| format!("Failed to write Pallets header: {e}"))?;
    worksheet
        .write_string_with_format(0, 2, headers[2], &header_format)
        .map_err(|e| format!("Failed to write Pallets header: {e}"))?;
    worksheet
        .write_string_with_format(0, 3, headers[3], &header_format)
        .map_err(|e| format!("Failed to write Pallets header: {e}"))?;
    worksheet
        .write_string_with_format(0, 4, headers[4], &header_format)
        .map_err(|e| format!("Failed to write Pallets header: {e}"))?;
    worksheet
        .write_string_with_format(0, 5, headers[5], &header_format)
        .map_err(|e| format!("Failed to write Pallets header: {e}"))?;
    worksheet
        .write_string_with_format(0, 6, headers[6], &header_number_format)
        .map_err(|e| format!("Failed to write Pallets header: {e}"))?;
    worksheet
        .write_string_with_format(0, 7, headers[7], &header_format)
        .map_err(|e| format!("Failed to write Pallets header: {e}"))?;

    let mut row_index: u32 = 1;
    let mut last_item_row: u32 = 0;

    for group in groups {
        let start_row = row_index;

        for row in &group.rows {
            worksheet
                .write_string_with_format(row_index, 0, &row.category, &hidden_value_format)
                .map_err(|e| format!("Failed to write Pallets category: {e}"))?;
            worksheet
                .write_string_with_format(row_index, 1, &row.subcategory, &hidden_value_format)
                .map_err(|e| format!("Failed to write Pallets subcategory: {e}"))?;
            worksheet
                .write_string_with_format(row_index, 2, &row.asin, &data_text_format)
                .map_err(|e| format!("Failed to write Pallets ASIN: {e}"))?;
            worksheet
                .write_string_with_format(row_index, 3, &row.item_description, &data_text_format)
                .map_err(|e| format!("Failed to write Pallets description: {e}"))?;
            worksheet
                .write_number_with_format(row_index, 4, row.qty, &data_text_format)
                .map_err(|e| format!("Failed to write Pallets qty: {e}"))?;
            worksheet
                .write_string_with_format(row_index, 5, &row.unit_retail_text, &data_text_format)
                .map_err(|e| format!("Failed to write Pallets unit retail: {e}"))?;
            worksheet
                .write_number_with_format(row_index, 6, row.ext_retail, &data_number_format)
                .map_err(|e| format!("Failed to write Pallets ext retail: {e}"))?;
            worksheet
                .write_string_with_format(row_index, 7, &row.pallet_id, &data_text_format)
                .map_err(|e| format!("Failed to write Pallets pallet id: {e}"))?;
            row_index += 1;
        }

        let end_row = row_index.saturating_sub(1);
        last_item_row = end_row;
        let summary_row = row_index;
        let excel_summary_row = summary_row + 1;
        let excel_start_row = start_row + 1;
        let excel_end_row = end_row + 1;
        let pallet_price = group.ext_total * 0.2;

        worksheet
            .write_blank(summary_row, 0, &summary_hidden_format)
            .map_err(|e| format!("Failed to write Pallets summary blank: {e}"))?;
        worksheet
            .write_blank(summary_row, 1, &summary_hidden_format)
            .map_err(|e| format!("Failed to write Pallets summary blank: {e}"))?;
        worksheet
            .write_string_with_format(summary_row, 2, "Price", &summary_text_format)
            .map_err(|e| format!("Failed to write Pallets summary label: {e}"))?;
        worksheet
            .write_formula_with_format(
                summary_row,
                3,
                Formula::new(format!("=G{excel_summary_row}*20%"))
                    .set_result(formula_result(pallet_price)),
                &summary_price_format,
            )
            .map_err(|e| format!("Failed to write Pallets summary price formula: {e}"))?;
        worksheet
            .write_formula_with_format(
                summary_row,
                4,
                Formula::new(format!("=SUM(E{excel_start_row}:E{excel_end_row})"))
                    .set_result(formula_result(group.qty_total)),
                &summary_number_format,
            )
            .map_err(|e| format!("Failed to write Pallets summary qty formula: {e}"))?;
        worksheet
            .write_blank(summary_row, 5, &summary_blank_format)
            .map_err(|e| format!("Failed to write Pallets summary blank: {e}"))?;
        worksheet
            .write_formula_with_format(
                summary_row,
                6,
                Formula::new(format!("=SUM(G{excel_start_row}:G{excel_end_row})"))
                    .set_result(formula_result(group.ext_total)),
                &summary_number_format,
            )
            .map_err(|e| format!("Failed to write Pallets summary total formula: {e}"))?;
        worksheet
            .write_string_with_format(summary_row, 7, &group.pallet_id, &summary_text_format)
            .map_err(|e| format!("Failed to write Pallets summary pallet id: {e}"))?;
        worksheet
            .set_row_height(summary_row, 18.2)
            .map_err(|e| format!("Failed to set Pallets summary row height: {e}"))?;

        row_index += 1;

        worksheet
            .write_blank(row_index, 0, &hidden_value_format)
            .map_err(|e| format!("Failed to write Pallets repeated header blank: {e}"))?;
        worksheet
            .write_blank(row_index, 1, &hidden_value_format)
            .map_err(|e| format!("Failed to write Pallets repeated header blank: {e}"))?;
        worksheet
            .write_string_with_format(row_index, 2, headers[2], &header_format)
            .map_err(|e| format!("Failed to write Pallets repeated header: {e}"))?;
        worksheet
            .write_string_with_format(row_index, 3, headers[3], &header_format)
            .map_err(|e| format!("Failed to write Pallets repeated header: {e}"))?;
        worksheet
            .write_string_with_format(row_index, 4, headers[4], &header_format)
            .map_err(|e| format!("Failed to write Pallets repeated header: {e}"))?;
        worksheet
            .write_string_with_format(row_index, 5, headers[5], &header_format)
            .map_err(|e| format!("Failed to write Pallets repeated header: {e}"))?;
        worksheet
            .write_string_with_format(row_index, 6, headers[6], &header_number_format)
            .map_err(|e| format!("Failed to write Pallets repeated header: {e}"))?;
        worksheet
            .write_string_with_format(row_index, 7, headers[7], &header_format)
            .map_err(|e| format!("Failed to write Pallets repeated header: {e}"))?;

        row_index += 1;
    }

    if last_item_row > 0 {
        worksheet
            .autofilter(0, 0, last_item_row, 7)
            .map_err(|e| format!("Failed to set Pallets autofilter: {e}"))?;
    }

    Ok(())
}

fn write_static_brands_block(worksheet: &mut Worksheet) -> Result<(), String> {
    let center_bold_border_wrap = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap();
    let border_text = Format::new().set_border(FormatBorder::Thin);
    let border_center = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);
    let border_number = Format::new()
        .set_border(FormatBorder::Thin)
        .set_num_format("#,##0");
    let border_percent = Format::new()
        .set_border(FormatBorder::Thin)
        .set_num_format("0%");
    let percent_two_decimals = Format::new().set_num_format("0.00%");

    worksheet
        .write_string(1, 4, "EBAY")
        .map_err(|e| format!("Failed to write Brands static value: {e}"))?;
    worksheet
        .write_string(1, 5, "SAMPLE")
        .map_err(|e| format!("Failed to write Brands static value: {e}"))?;

    worksheet
        .write_string_with_format(2, 5, "Marketplace", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands static header: {e}"))?;
    worksheet
        .write_string_with_format(2, 6, "category", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands static header: {e}"))?;
    worksheet
        .write_string_with_format(2, 7, "q-ty", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands static header: {e}"))?;
    worksheet
        .write_string_with_format(2, 8, "MSRP", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands static header: {e}"))?;
    worksheet
        .write_string_with_format(2, 9, "retail sale", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands static header: {e}"))?;
    worksheet
        .write_string_with_format(2, 10, "revenue", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands static header: {e}"))?;
    worksheet
        .write_string_with_format(2, 11, "cost", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands static header: {e}"))?;
    worksheet
        .write_string_with_format(2, 12, "profit/loss", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands static header: {e}"))?;

    let marketplaces = [
        ("EBAY", "from 200", 73.0, 41582.0, 0.35),
        ("HIBID", "from 100 to 200", 200.0, 56042.0, 0.22),
        ("PALLET", "under 100", 258.0, 23140.0, 0.10),
    ];

    for (offset, (marketplace, category, qty, msrp, retail_sale)) in marketplaces.iter().enumerate() {
        let row = offset as u32 + 3;
        let revenue = *msrp * *retail_sale;
        let cost = *msrp * 0.15_f64;
        let profit_loss = revenue - cost;
        let profit_pct = if cost.abs() < f64::EPSILON {
            0.0
        } else {
            profit_loss / cost
        };

        worksheet
            .write_string_with_format(row, 5, *marketplace, &border_text)
            .map_err(|e| format!("Failed to write Brands marketplace: {e}"))?;
        worksheet
            .write_string_with_format(row, 6, *category, &border_text)
            .map_err(|e| format!("Failed to write Brands category: {e}"))?;
        worksheet
            .write_number_with_format(row, 7, *qty, &border_text)
            .map_err(|e| format!("Failed to write Brands qty: {e}"))?;
        worksheet
            .write_number_with_format(row, 8, *msrp, &border_number)
            .map_err(|e| format!("Failed to write Brands MSRP: {e}"))?;
        worksheet
            .write_number_with_format(row, 9, *retail_sale, &border_percent)
            .map_err(|e| format!("Failed to write Brands sale pct: {e}"))?;
        worksheet
            .write_formula_with_format(
                row,
                10,
                Formula::new(format!("=I{}*J{}", row + 1, row + 1)).set_result(formula_result(revenue)),
                &border_number,
            )
            .map_err(|e| format!("Failed to write Brands revenue formula: {e}"))?;
        worksheet
            .write_formula_with_format(
                row,
                11,
                Formula::new(format!("=I{}*15%", row + 1)).set_result(formula_result(cost)),
                &border_number,
            )
            .map_err(|e| format!("Failed to write Brands cost formula: {e}"))?;
        worksheet
            .write_formula_with_format(
                row,
                12,
                Formula::new(format!("=K{}-L{}", row + 1, row + 1)).set_result(formula_result(profit_loss)),
                &border_number,
            )
            .map_err(|e| format!("Failed to write Brands profit formula: {e}"))?;
        worksheet
            .write_formula_with_format(
                row,
                15,
                Formula::new(format!("=M{0}/L{0}", row + 1)).set_result(formula_result(profit_pct)),
                &border_number,
            )
            .map_err(|e| format!("Failed to write Brands profit pct formula: {e}"))?;
    }

    worksheet
        .write_formula_with_format(6, 7, Formula::new("=SUM(H4:H6)").set_result("531"), &border_number)
        .map_err(|e| format!("Failed to write Brands total qty formula: {e}"))?;
    worksheet
        .write_formula_with_format(6, 8, Formula::new("=SUM(I4:I6)").set_result("120764"), &border_number)
        .map_err(|e| format!("Failed to write Brands total MSRP formula: {e}"))?;
    worksheet
        .write_formula_with_format(
            6,
            12,
            Formula::new("=SUM(M4:M6)").set_result("11082.34"),
            &border_number,
        )
        .map_err(|e| format!("Failed to write Brands total profit formula: {e}"))?;
    worksheet
        .write_formula_with_format(7, 8, Formula::new("=I7").set_result("120764"), &border_number)
        .map_err(|e| format!("Failed to write Brands helper formula: {e}"))?;
    worksheet
        .write_number_with_format(8, 13, 0.015, &percent_two_decimals)
        .map_err(|e| format!("Failed to write Brands fee pct: {e}"))?;

    worksheet
        .write_string_with_format(9, 5, "Truckload name", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands truckload header: {e}"))?;
    worksheet
        .write_string_with_format(9, 6, "cost", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands truckload header: {e}"))?;
    worksheet
        .write_string_with_format(9, 7, "q-ty", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands truckload header: {e}"))?;
    worksheet
        .write_string_with_format(9, 8, "retail price", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands truckload header: {e}"))?;
    worksheet
        .write_string_with_format(9, 9, "pallet cost", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands truckload header: {e}"))?;
    worksheet
        .write_string_with_format(9, 10, "pallet MSRP", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands truckload header: {e}"))?;
    worksheet
        .write_string_with_format(9, 11, "pallet sale price", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands truckload header: {e}"))?;
    worksheet
        .write_string_with_format(9, 12, "total revenue", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands truckload header: {e}"))?;
    worksheet
        .write_string_with_format(9, 13, "Sales fee", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands truckload header: {e}"))?;
    worksheet
        .write_string_with_format(9, 14, "sales tax", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands truckload header: {e}"))?;
    worksheet
        .write_string_with_format(9, 15, "total profit", &center_bold_border_wrap)
        .map_err(|e| format!("Failed to write Brands truckload header: {e}"))?;

    let truckloads = [
        ("FBA Kitchen Goods (Medium)", 17875.18, 881.0, 120764.0),
        (
            "Home Goods, Toys & More by iRobot, Drive Medical (medium)",
            9528.83,
            646.0,
            64062.0,
        ),
    ];

    for (offset, (name, cost, qty, retail)) in truckloads.iter().enumerate() {
        let row = offset as u32 + 10;
        let pallet_cost = *cost / 24.0_f64;
        let pallet_msrp = *retail / 24.0_f64;
        let pallet_sale_price = (pallet_cost * 1.3) * 1.05;
        let total_revenue = pallet_sale_price * 24.0;
        let sales_fee = total_revenue * 0.015;
        let sales_tax = total_revenue * 0.0825;
        let total_profit = total_revenue - *cost - sales_tax;

        worksheet
            .write_string_with_format(row, 5, *name, &border_text)
            .map_err(|e| format!("Failed to write Brands truckload name: {e}"))?;
        worksheet
            .write_number_with_format(row, 6, *cost, &border_number)
            .map_err(|e| format!("Failed to write Brands truckload cost: {e}"))?;
        worksheet
            .write_number_with_format(row, 7, *qty, &border_center)
            .map_err(|e| format!("Failed to write Brands truckload qty: {e}"))?;
        worksheet
            .write_number_with_format(row, 8, *retail, &border_number)
            .map_err(|e| format!("Failed to write Brands truckload retail: {e}"))?;
        worksheet
            .write_formula_with_format(
                row,
                9,
                Formula::new(format!("=G{0}/24", row + 1)).set_result(formula_result(pallet_cost)),
                &border_number,
            )
            .map_err(|e| format!("Failed to write Brands pallet cost formula: {e}"))?;
        worksheet
            .write_formula_with_format(
                row,
                10,
                Formula::new(format!("=I{0}/24", row + 1)).set_result(formula_result(pallet_msrp)),
                &border_number,
            )
            .map_err(|e| format!("Failed to write Brands pallet MSRP formula: {e}"))?;
        worksheet
            .write_formula_with_format(
                row,
                11,
                Formula::new(format!("=(J{0}*1.3)*1.05", row + 1))
                    .set_result(formula_result(pallet_sale_price)),
                &border_number,
            )
            .map_err(|e| format!("Failed to write Brands pallet sale formula: {e}"))?;
        worksheet
            .write_formula_with_format(
                row,
                12,
                Formula::new(format!("=L{0}*24", row + 1)).set_result(formula_result(total_revenue)),
                &border_number,
            )
            .map_err(|e| format!("Failed to write Brands revenue formula: {e}"))?;
        worksheet
            .write_formula_with_format(
                row,
                13,
                Formula::new(format!("=M{0}*$N$9", row + 1)).set_result(formula_result(sales_fee)),
                &border_number,
            )
            .map_err(|e| format!("Failed to write Brands fee formula: {e}"))?;
        worksheet
            .write_formula_with_format(
                row,
                14,
                Formula::new(format!("=M{0}*8.25%", row + 1)).set_result(formula_result(sales_tax)),
                &border_center,
            )
            .map_err(|e| format!("Failed to write Brands tax formula: {e}"))?;
        worksheet
            .write_formula_with_format(
                row,
                15,
                Formula::new(format!("=M{0}-G{0}-O{0}", row + 1)).set_result(formula_result(total_profit)),
                &border_center,
            )
            .map_err(|e| format!("Failed to write Brands profit formula: {e}"))?;
    }

    worksheet
        .write_string_with_format(12, 5, "Total", &border_text)
        .map_err(|e| format!("Failed to write Brands total label: {e}"))?;
    worksheet
        .write_formula_with_format(
            12,
            12,
            Formula::new("=SUM(M11:M12)").set_result("37389.58245"),
            &border_number,
        )
        .map_err(|e| format!("Failed to write Brands total revenue formula: {e}"))?;
    worksheet
        .write_formula_with_format(
            12,
            13,
            Formula::new("=SUM(N11:N12)").set_result("560.84373675"),
            &border_number,
        )
        .map_err(|e| format!("Failed to write Brands total fee formula: {e}"))?;
    worksheet
        .write_formula_with_format(
            12,
            14,
            Formula::new("=SUM(O11:O12)").set_result("3084.640552125"),
            &border_center,
        )
        .map_err(|e| format!("Failed to write Brands total tax formula: {e}"))?;
    worksheet
        .write_formula_with_format(
            12,
            15,
            Formula::new("=SUM(P11:P12)").set_result("6900.602727875"),
            &border_center,
        )
        .map_err(|e| format!("Failed to write Brands total profit formula: {e}"))?;

    worksheet
        .set_row_height(9, 30.05)
        .map_err(|e| format!("Failed to set Brands row height: {e}"))?;
    worksheet
        .set_row_height(11, 30.05)
        .map_err(|e| format!("Failed to set Brands row height: {e}"))?;

    Ok(())
}

fn write_brands_sheet(workbook: &mut Workbook, brands: &[BrandSummary]) -> Result<(), String> {
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name("Brands")
        .map_err(|e| format!("Failed to set Brands name: {e}"))?;
    worksheet.set_hidden(true).set_zoom(90);

    worksheet
        .set_column_width(0, 28.88671875)
        .map_err(|e| format!("Failed to set Brands width: {e}"))?;
    worksheet
        .set_column_width(2, 9.88671875)
        .map_err(|e| format!("Failed to set Brands width: {e}"))?;
    worksheet
        .set_column_width(3, 8.88671875)
        .map_err(|e| format!("Failed to set Brands width: {e}"))?;
    worksheet
        .set_column_width(5, 33.33203125)
        .map_err(|e| format!("Failed to set Brands width: {e}"))?;
    worksheet
        .set_column_width(6, 7.88671875)
        .map_err(|e| format!("Failed to set Brands width: {e}"))?;
    worksheet
        .set_column_width(7, 5.88671875)
        .map_err(|e| format!("Failed to set Brands width: {e}"))?;
    worksheet
        .set_column_width(8, 7.77734375)
        .map_err(|e| format!("Failed to set Brands width: {e}"))?;
    worksheet
        .set_column_width(9, 6.44140625)
        .map_err(|e| format!("Failed to set Brands width: {e}"))?;
    worksheet
        .set_column_width(10, 8.0)
        .map_err(|e| format!("Failed to set Brands width: {e}"))?;
    worksheet
        .set_column_width(11, 11.88671875)
        .map_err(|e| format!("Failed to set Brands width: {e}"))?;
    worksheet
        .set_column_width(12, 9.6640625)
        .map_err(|e| format!("Failed to set Brands width: {e}"))?;
    worksheet
        .set_column_width(13, 9.6640625)
        .map_err(|e| format!("Failed to set Brands width: {e}"))?;
    worksheet
        .set_column_width(14, 9.6640625)
        .map_err(|e| format!("Failed to set Brands width: {e}"))?;

    let header_text_format = Format::new().set_bold();
    let header_number_format = Format::new().set_bold().set_num_format("#,##0");
    let brand_number_format = Format::new().set_num_format("#,##0");
    let avg_formula_format = Format::new()
        .set_num_format("#,##0")
        .set_background_color(Color::RGB(0xD9E1F2));

    worksheet
        .write_blank(0, 0, &Format::new())
        .map_err(|e| format!("Failed to write Brands blank: {e}"))?;
    worksheet
        .write_string_with_format(0, 1, "Sum of Qty", &header_text_format)
        .map_err(|e| format!("Failed to write Brands header: {e}"))?;
    worksheet
        .write_string_with_format(0, 2, "Sum of Ext, Retail", &header_number_format)
        .map_err(|e| format!("Failed to write Brands header: {e}"))?;
    worksheet
        .write_string_with_format(0, 3, "avg price", &header_number_format)
        .map_err(|e| format!("Failed to write Brands header: {e}"))?;

    for (index, brand) in brands.iter().enumerate() {
        let row = index as u32 + 1;
        let avg_price = if brand.qty_total.abs() < f64::EPSILON {
            0.0
        } else {
            brand.ext_total / brand.qty_total
        };

        worksheet
            .write_string(row, 0, &brand.brand)
            .map_err(|e| format!("Failed to write Brands brand: {e}"))?;
        worksheet
            .write_number(row, 1, brand.qty_total)
            .map_err(|e| format!("Failed to write Brands qty: {e}"))?;
        worksheet
            .write_number_with_format(row, 2, brand.ext_total, &brand_number_format)
            .map_err(|e| format!("Failed to write Brands retail: {e}"))?;
        worksheet
            .write_formula_with_format(
                row,
                3,
                Formula::new(format!("=C{0}/B{0}", row + 1)).set_result(formula_result(avg_price)),
                &avg_formula_format,
            )
            .map_err(|e| format!("Failed to write Brands avg formula: {e}"))?;
    }

    worksheet
        .autofilter(0, 0, 0, 3)
        .map_err(|e| format!("Failed to set Brands autofilter: {e}"))?;

    write_static_brands_block(worksheet)?;

    Ok(())
}

fn generate_flat_workbook(
    file_path: &str,
    save_path: &str,
) -> Result<PalletManifestExportResult, String> {
    let raw_lines = read_raw_lines(file_path)?;
    let rows = parse_pallet_manifest_csv(file_path)?;
    let groups = build_pallet_groups(&rows);
    let brands = build_brand_summaries(&rows);
    let output_path = finalize_output_path(save_path);

    ensure_parent_dir(&output_path)?;

    let mut workbook = Workbook::new();
    let raw_sheet_name = build_raw_sheet_name(file_path);

    write_raw_sheet(&mut workbook, &raw_sheet_name, &raw_lines)?;
    write_list_sheet(&mut workbook, &groups)?;
    write_sheet3(&mut workbook, &brands)?;
    write_sheet2(&mut workbook, &groups)?;
    write_sheet4(&mut workbook, &groups)?;
    write_pallets_sheet(&mut workbook, &groups)?;
    write_brands_sheet(&mut workbook, &brands)?;

    workbook
        .save(&output_path)
        .map_err(|e| format!("Failed to save pallet manifest workbook: {e}"))?;

    Ok(PalletManifestExportResult {
        file_path: output_path,
        items_count: rows.len(),
        pallets_count: groups.len(),
    })
}

fn generate_workbook(file_path: &str, save_path: &str) -> Result<PalletManifestExportResult, String> {
    generate_flat_workbook(file_path, save_path)
}

#[tauri::command]
pub fn generate_pallet_manifest_report(
    file_path: String,
    save_path: String,
) -> Result<PalletManifestExportResult, String> {
    generate_workbook(&file_path, &save_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_row(pallet_id: &str, brand: &str, asin: &str, ext_retail: f64) -> PalletManifestRow {
        PalletManifestRow {
            category: "Home".to_string(),
            subcategory: "Kitchen".to_string(),
            asin: asin.to_string(),
            item_description: format!("Item {asin}"),
            qty: 1.0,
            unit_retail_text: format!("{ext_retail:.2}"),
            ext_retail,
            brand: brand.to_string(),
            pallet_id: pallet_id.to_string(),
        }
    }

    #[test]
    fn build_pallet_groups_sorts_pallets_and_preserves_row_order() {
        let rows = vec![
            make_row("PAL-2", "B", "A2", 10.0),
            make_row("PAL-1", "A", "A1", 20.0),
            make_row("PAL-2", "B", "A3", 30.0),
            make_row("PAL-1", "A", "A4", 40.0),
        ];

        let groups = build_pallet_groups(&rows);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].pallet_id, "PAL-1");
        assert_eq!(groups[0].rows[0].asin, "A1");
        assert_eq!(groups[0].rows[1].asin, "A4");
        assert_eq!(groups[1].pallet_id, "PAL-2");
        assert_eq!(groups[1].rows[0].asin, "A2");
        assert_eq!(groups[1].rows[1].asin, "A3");
    }

    #[test]
    fn build_brand_summaries_groups_blank_brand_and_sorts_desc() {
        let rows = vec![
            make_row("PAL-1", "", "A1", 10.0),
            make_row("PAL-1", "Zed", "A2", 90.0),
            make_row("PAL-2", "Acme", "A3", 100.0),
            make_row("PAL-2", "Acme", "A4", 25.0),
        ];

        let brands = build_brand_summaries(&rows);

        assert_eq!(brands[0].brand, "Acme");
        assert_eq!(brands[0].qty_total, 2.0);
        assert!((brands[0].ext_total - 125.0).abs() < 0.001);
        assert_eq!(brands[1].brand, "Zed");
        assert_eq!(brands[2].brand, "(blank)");
    }

    #[test]
    fn generate_workbook_creates_file_for_reference_csv() {
        let csv_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("palet_ref")
            .join("BStock_Fast Shipping - 6 Pallets of FBA Home Goods_Manifest.csv");
        let output_dir =
            std::env::temp_dir().join(format!("sugarland_pallet_manifest_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&output_dir).expect("failed to create output dir");
        let output_path = output_dir.join("manifest.xlsx");

        let result = generate_workbook(
            csv_path.to_str().expect("csv path"),
            output_path.to_str().expect("output path"),
        )
        .expect("workbook should be generated");

        assert!(Path::new(&result.file_path).exists());
        assert_eq!(result.pallets_count, 6);
        assert_eq!(result.items_count, 198);
    }

    #[test]
    fn generate_workbook_from_env_paths() {
        let csv_path = match std::env::var("PALLET_CSV_PATH") {
            Ok(value) => value,
            Err(_) => return,
        };
        let output_path = match std::env::var("PALLET_OUT_PATH") {
            Ok(value) => value,
            Err(_) => return,
        };

        let result = generate_workbook(&csv_path, &output_path)
            .expect("workbook should be generated from env paths");

        println!("GENERATED_FILE={}", result.file_path);
        println!("GENERATED_ITEMS={}", result.items_count);
        println!("GENERATED_PALLETS={}", result.pallets_count);
    }
}
