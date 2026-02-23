import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type {
    ManifestSummary,
    InventoryItem,
    Auction,
    ReconciliationSummary,
    DashboardStats,
    Vendor,
    ProfitLossReport,
    AuctionPnlRow,
    ItemStatus,
    ValidationResult
} from '@/types';

// Wrap invoke to handle mock mode if backend is not available (for dev without Rust)
const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function invokeCommand<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
    if (isTauri) {
        return invoke<T>(cmd, args);
    }

    // Mock fallback for browser dev
    console.log(`[Mock] Invoking ${cmd}`, args);
    return mockResponse(cmd);
}

// ----------------------------------------------------------------------------
// API Methods
// ----------------------------------------------------------------------------

export const api = {
    // File Dialog
    selectFile: async (filters?: any[]) => {
        if (isTauri) {
            return open({ multiple: false, directory: false, filters });
        }
        return 'C:\\mock\\path\\to\\file.csv';
    },

    saveFile: async (defaultName?: string) => {
        if (isTauri) {
            const { save } = await import('@tauri-apps/plugin-dialog');
            return save({
                defaultPath: defaultName,
                filters: [{ name: 'CSV', extensions: ['csv'] }, { name: 'Excel', extensions: ['xlsx'] }],
            });
        }
        return `C:\\mock\\exports\\${defaultName ?? 'export.csv'}`;
    },

    saveBinaryFile: async (filePath: string, data: Uint8Array) => {
        if (isTauri) {
            return invokeCommand<void>('save_binary_file', { filePath, data: Array.from(data) });
        }
        console.log(`[Mock] Saving binary file to ${filePath}`);
    },

    // Manifests & Inventory
    importManifest: (file_path: string, auction_id?: string) =>
        invokeCommand<ManifestSummary>('import_manifest', { filePath: file_path, auctionId: auction_id }),

    getInventoryItems: (status?: string) =>
        invokeCommand<InventoryItem[]>('get_inventory_items', { status }),

    getDashboardStats: () =>
        invokeCommand<DashboardStats>('get_dashboard_stats'),

    getVendors: () =>
        invokeCommand<Vendor[]>('get_vendors'),

    // Auctions
    createAuction: (req: { name: string; vendor_id?: string; start_date?: string; end_date?: string }) =>
        invokeCommand<string>('create_auction', { req }),

    getAuctions: () =>
        invokeCommand<Auction[]>('get_auctions'),

    getAuctionById: (auctionId: string) =>
        invokeCommand<Auction>('get_auction_by_id', { auctionId }),

    updateAuctionStatus: (auctionId: string, status: string) =>
        invokeCommand<void>('update_auction_status', { auctionId, status }),

    updateVendor: (vendorId: string, data: { cost_coefficient: number; min_price_margin: number }) =>
        invokeCommand<void>('update_vendor', { vendorId, data }),

    exportAuctionCsv: (auctionId: string, filePath: string) =>
        invokeCommand<number>('export_auction_csv', { auctionId, filePath }),

    // Reconciliation
    reconcileAuction: (auctionId: string, filePath: string) =>
        invokeCommand<ReconciliationSummary>('reconcile_auction', { auctionId, filePath }),

    getPlReport: () =>
        invokeCommand<ProfitLossReport>('get_pl_report'),
    getSettings: (key: string) =>
        invokeCommand<string | null>('get_setting', { key }),

    saveSetting: (key: string, value: string) =>
        invokeCommand<void>('save_setting', { key, value }),

    updateItemStatus: (itemId: string, status: ItemStatus) =>
        invokeCommand<void>('update_item_status', { itemId, status }),

    exportInventoryCsv: (filePath: string, status?: string) =>
        invokeCommand<number>('export_inventory_csv', { filePath, status }),

    getAuctionPnlList: () =>
        invokeCommand<AuctionPnlRow[]>('get_auction_pnl_list'),

    unassignItem: (itemId: string) =>
        invokeCommand<void>('unassign_item', { itemId }),

    validateCsv: (filePath: string) =>
        invokeCommand<ValidationResult>('validate_csv', { filePath }),
};

// ----------------------------------------------------------------------------
// Mocks
// ----------------------------------------------------------------------------

function mockResponse(cmd: string): any {
    switch (cmd) {
        case 'get_dashboard_stats':
            return {
                total_items: 1250,
                in_stock: 850,
                listed: 200,
                sold: 150,
                buyback: 50,
                total_retail_value: 125000.00,
                total_cost: 15000.00,
                active_auctions: 2,
            };
        case 'get_vendors':
            return [
                { id: 'bestbuy', name: 'Best Buy', cost_coefficient: 0.14, min_price_margin: 0.10, is_active: true, created_at: new Date().toISOString() },
                { id: 'wayfair', name: 'Wayfair', cost_coefficient: 0.07, min_price_margin: 0.10, is_active: true, created_at: new Date().toISOString() },
                { id: 'mech', name: 'Mech/PDX7', cost_coefficient: 0.20, min_price_margin: 0.10, is_active: true, created_at: new Date().toISOString() },
                { id: 'amazon', name: 'Amazon Bstock', cost_coefficient: 0.20, min_price_margin: 0.10, is_active: true, created_at: new Date().toISOString() },
            ];
        case 'get_inventory_items':
            return Array(10).fill(null).map((_, i) => ({
                id: `item-${i}`,
                raw_title: `Mock Item ${i}`,
                lot_number: `LOT-${1000 + i}`,
                retail_price: 100.0 + i * 10,
                cost_price: 10.0 + i,
                min_price: 50.0 + i * 5,
                current_status: ['InStock', 'Listed', 'Sold', 'Buyback'][i % 4],
                source: 'Best Buy',
                created_at: new Date().toISOString(),
            }));
        case 'get_auctions':
            return [
                { id: '1', name: 'Weekly Auction #45', status: 'Draft', total_lots: 0, created_at: new Date().toISOString() },
                { id: '2', name: 'Electronics Clearance', status: 'Active', total_lots: 50, created_at: new Date().toISOString() },
                { id: '3', name: 'Furniture Liquidation', status: 'Completed', total_lots: 120, created_at: new Date().toISOString() },
            ];
        case 'get_auction_by_id':
            return { id: '1', name: 'Weekly Auction #45', status: 'Draft', total_lots: 0, created_at: new Date().toISOString() };
        case 'update_auction_status':
        case 'update_vendor':
            return null;
        case 'create_auction':
            return 'mock-auction-id';
        case 'import_manifest':
            return {
                id: 'mock-manifest',
                items_count: 24,
                total_retail: 1200.0,
                total_cost: 120.0
            };
        case 'reconcile_auction':
            return {
                sold_count: 10,
                buyback_count: 2,
                total_revenue: 1000.0,
                total_profit: 500.0,
                errors: []
            };
        case 'get_pl_report':
            return {
                total_revenue: 50000,
                total_cogs: 10000,
                gross_profit: 40000,
                total_expenses: 5000,
                net_profit: 35000,
                margin_percent: 70,
                sold_items: 500
            };
        case 'get_auction_pnl_list':
            return [
                { auction_id: '1', auction_name: 'Weekly #43', sold_items: 80, buyback_items: 10, total_revenue: 12000, total_cost: 4000, total_commission: 1800, net_profit: 6200, total_items: 100 },
                { auction_id: '2', auction_name: 'Electronics #12', sold_items: 45, buyback_items: 5, total_revenue: 8500, total_cost: 2800, total_commission: 1275, net_profit: 4425, total_items: 60 },
                { auction_id: '3', auction_name: 'Furniture Lot', sold_items: 30, buyback_items: 8, total_revenue: 5200, total_cost: 1500, total_commission: 780, net_profit: 2920, total_items: 45 },
            ];
        case 'export_inventory_csv':
            return 10;
        case 'update_item_status':
        case 'save_setting':
        case 'unassign_item':
            return null;
        case 'get_setting':
            return "0.15";
        case 'validate_csv':
            return { valid: true, message: 'CSV is valid. Checked 5 rows.', warnings: [] };
        default:
            return null;
    }
}
