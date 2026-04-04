// ============================================================
// Sugarland — TypeScript Type Definitions
// ============================================================

// --- Vendor ---
export interface Vendor {
    id: string;
    name: string;
    cost_coefficient: number;
    min_price_margin: number;
    is_active: boolean;
    created_at: string;
}

// --- Manifest ---
export interface Manifest {
    id: string;
    import_date: string;
    source_filename: string;
    total_retail_value: number;
    total_cost: number;
    items_count: number;
    status: ManifestStatus;
    notes?: string;
}

export type ManifestStatus = 'Imported' | 'Listed' | 'Closed';

// --- Inventory Item ---
export interface InventoryItem {
    id: string;
    manifest_id: string;
    lot_number?: string;
    quantity: number;

    // Raw data
    raw_title: string;
    vendor_code?: string;
    source?: string;
    condition?: string;
    read_description_flag?: boolean;

    // Normalized data
    normalized_title?: string;
    extracted_brand?: string;
    extracted_model?: string;
    sku_extracted?: string;
    category?: string;

    // Financial data
    retail_price: number;
    cost_price: number;
    min_price: number;

    // Status
    current_status: ItemStatus;

    // Auction
    auction_id?: string;
    listed_at?: string;
    sold_at?: string;

    // HiBid sync / Buy-backer
    sale_order?: number;
    buybacker_id?: string;

    created_at: string;
    updated_at: string;
}

export type ItemStatus = 'InStock' | 'Listed' | 'Sold' | 'Unsold' | 'FloorSale' | 'Buyback' | 'Scrap';

// --- Auction ---
export interface Auction {
    id: string;
    hibid_auction_id?: string;
    name: string;
    vendor_id?: string;
    start_date?: string;
    end_date?: string;
    status: AuctionStatus;
    total_lots: number;
    created_at: string;
}

export type AuctionStatus = 'Active' | 'Completed';

// --- Auction Result ---
export interface AuctionResult {
    id: string;
    auction_id: string;
    item_id: string;
    winning_bidder: string;
    bidder_id: string;
    high_bid: number;
    max_bid?: number;
    bidder_email?: string;
    bidder_phone?: string;
    is_buyback: boolean;
    is_paid: boolean;
    commission_rate: number;
    commission_amount: number;
    net_profit: number;
    created_at: string;
}

export interface AuctionResultBid {
    item_id: string;
    high_bid: number;
}

export interface AuctionPnlRow {
    auction_id: string;
    auction_name: string;
    start_date?: string;
    total_items: number;
    sold_items: number;
    buyback_items: number;
    total_revenue: number;
    total_cost: number;
    total_commission: number;
    net_profit: number;
}

// --- Historical Sales ---
export interface HistoricalSale {
    id: string;
    normalized_title: string;
    extracted_brand?: string;
    extracted_sku?: string;
    category?: string;
    condition?: string;
    retail_price: number;
    cost_price: number;
    sale_price: number;
    sale_date: string;
    platform: string;
    season: string;
    created_at: string;
}

// --- CSV Import ---
export interface BStockManifestRow {
    'Auction name': string;
    LotNumber: string;
    Quantity: string;
    Title: string;
    'Vendor Code': string;
    'Retail Price': string;
    Source: string;
    'Read Description Flag'?: string;
}

export interface HiBidExportRow {
    LotNum: string;
    Lead: string;
    Description: string;
    StartBid: string;
    Images: string;
}

// --- Pricing ---
export interface PricingInput {
    retail_price: number;
    source: string;
    vendor?: Vendor;
}

export interface PricingOutput {
    cost_price: number;
    min_price: number;
    coefficient_used: number;
    vendor_matched: string;
}

// --- Reports ---
export interface PLReport {
    auction_name: string;
    period_start: string;
    period_end: string;
    total_lots: number;
    sold_count: number;
    buyback_count: number;
    unsold_count: number;
    gross_sales: number;
    total_commissions: number;
    net_revenue: number;
    total_cost: number;
    net_profit: number;
    profit_margin: number;
    by_source: SourceBreakdown[];
}

export interface SourceBreakdown {
    source: string;
    items_count: number;
    revenue: number;
    cost: number;
    profit: number;
}

// --- Manifest Import Summary ---
export interface ManifestSummary {
    id: string;
    items_count: number;
    total_retail: number;
    total_cost: number;
}

// --- Reconciliation ---
export interface ReconciliationSummary {
    sold_count: number;
    buyback_count: number;
    total_revenue: number;
    total_profit: number;
    errors: string[];
}

// --- Dashboard Stats ---
export interface DashboardStats {
    total_items: number;
    in_stock: number;
    listed: number;
    sold: number;
    buyback: number;
    total_retail_value: number;
    total_cost: number;
    active_auctions: number;
}

// --- Profit Loss Report ---
export interface ProfitLossReport {
    total_revenue: number;
    total_cogs: number;
    gross_profit: number;
    total_expenses: number;
    net_profit: number;
    margin_percent: number;
    sold_items: number;
    total_lots: number;
    buyback_count: number;
    unsold_count: number;
    sell_through_rate: number;
    avg_sale_price: number;
    period_label: string;
}

export interface AuctionSummary {
    auction_id: string;
    auction_name: string;
    completed_at: string;
    total_lots: number;
    sold_count: number;
    buyback_count: number;
    unsold_count: number;
    total_revenue: number;
    total_cogs: number;
    total_commission: number;
    net_profit: number;
    plus_bonus: number;
    margin_percent: number;
}

export interface VendorBreakdown {
    source: string;
    item_count: number;
    total_retail: number;
    total_cost: number;
    cost_pct: number;
    total_revenue: number;
    revenue_with_commission: number;
    revenue_pct: number;
    profit_loss: number;
}

// --- Filters ---
export interface InventoryFilter {
    status?: ItemStatus;
    source?: string;
    manifest_id?: string;
    search?: string;
}

export interface ValidationResult {
    valid: boolean;
    message: string;
    warnings: string[];
}

// --- Auction Reports ---
export interface AuctionReport {
    id: string;
    auction_id: string;
    auction_name: string;
    report_type: 'detail' | 'summary';
    file_name: string;
    file_path: string;
    created_at: string;
}

export interface FinishAuctionResult {
    detail_report: string;
    summary_report: string;
}

export interface PalletManifestExportResult {
    file_path: string;
    items_count: number;
    pallets_count: number;
}

// --- Condition Types ---
export interface ConditionType {
    id: string;
    label: string;
    category: string;
}

// --- Source Types ---
export interface SourceType {
    id: string;
    name: string;
}

// --- Pricing Rules ---
export interface PricingRule {
    id: number;
    condition_category: string;
    level: number;
    multiplier: number;
    label?: string;
}

// --- Buy-backer ---
export interface Buybacker {
    id: string;
    name: string;
    is_active: boolean;
}

// --- Item History (Repeaters) ---
export interface ItemHistoryEntry {
    auction_name: string;
    lot_number?: string;
    high_bid: number;
    sale_date: string;
    bidder_name: string;
    is_buyback: boolean;
}
