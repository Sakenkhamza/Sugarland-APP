import { useEffect, useState, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { api } from '@/lib/api';
import type { Auction, InventoryItem, ConditionType, Buybacker, ItemHistoryEntry, Vendor } from '@/types';
import { ArrowLeft, X, Search, Flag, FileSpreadsheet, Calculator, History, RefreshCw, PlusCircle, AlertCircle, Trash2 } from 'lucide-react';
import * as XLSX from 'xlsx';
import { SortableTableHead } from '@/components/ui/sortable-table-head';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
} from '@/components/ui/table';
import { formatCurrencyWhole, naturalSort } from '@/lib/utils';
import { toast } from 'sonner';

// ============================================================
// Inline Select Component
// ============================================================
function InlineSelect({ value, options, onChange, placeholder, className }: {
    value: string;
    options: { value: string; label: string }[];
    onChange: (val: string) => void;
    placeholder?: string;
    className?: string;
}) {
    return (
        <select
            className={`w-full bg-transparent border border-transparent hover:border-border rounded px-1 py-0.5 text-xs cursor-pointer focus:outline-none focus:ring-1 focus:ring-ring transition-colors ${className || ''}`}
            value={value}
            onChange={(e) => onChange(e.target.value)}
        >
            {placeholder && <option value="" className="text-foreground">{placeholder}</option>}
            {options.map(opt => (
                <option key={opt.value} value={opt.value} className="text-foreground">{opt.label}</option>
            ))}
        </select>
    );
}

// ============================================================
// Main Component
// ============================================================
export function AuctionDetailPage() {
    const { id } = useParams<{ id: string }>();
    const navigate = useNavigate();

    const [auction, setAuction] = useState<Auction | null>(null);
    const [auctionItems, setAuctionItems] = useState<InventoryItem[]>([]);
    const [loading, setLoading] = useState(true);

    // Reference data
    const [conditionTypes, setConditionTypes] = useState<ConditionType[]>([]);
    const [buybackers, setBuybackers] = useState<Buybacker[]>([]);
    const [vendors, setVendors] = useState<Vendor[]>([]);

    // Pricing calculator
    const [vendorCosts, setVendorCosts] = useState<Record<string, number>>({});
    const [conditionMargins, setConditionMargins] = useState<Record<string, number>>({});
    const [showPricingPanel, setShowPricingPanel] = useState(false);
    const [newSupplierName, setNewSupplierName] = useState('');

    // History popover
    const [historyItem, setHistoryItem] = useState<string | null>(null);
    const [historyEntries, setHistoryEntries] = useState<ItemHistoryEntry[]>([]);
    const [historyLoading, setHistoryLoading] = useState(false);

    // Filters
    const [searchQuery, setSearchQuery] = useState('');
    const [statusFilter, setStatusFilter] = useState('all');
    const [vendorFilter, setVendorFilter] = useState('all');
    const [sortConfig, setSortConfig] = useState<{ column: string; direction: 'asc' | 'desc'; pinnedValue?: string } | null>({ column: 'lot_number', direction: 'asc' });
    const handleSort = useCallback((column: string, direction: 'asc' | 'desc', pinnedValue?: string) => {
        setSortConfig({ column, direction, pinnedValue });
    }, []);

    const [isUpdatingStatus, setIsUpdatingStatus] = useState(false);

    useEffect(() => {
        loadReferenceData();
    }, []);

    useEffect(() => {
        if (id) loadAuctionData(id);
    }, [id]);

    const loadReferenceData = async () => {
        try {
            const [ct, bb, vs] = await Promise.all([
                api.getConditionTypes(),
                api.getBuybackers(),
                api.getVendors(),
            ]);
            setConditionTypes(ct);
            setBuybackers(bb);
            setVendors(vs);
        } catch (err) {
            console.error('Failed to load reference data:', err);
        }
    };

    const loadAuctionData = async (auctionId: string) => {
        try {
            setLoading(true);
            const auc = await api.getAuctionById(auctionId);
            setAuction(auc);

            const allItems = await api.getInventoryItems();
            setAuctionItems(allItems.filter(item => item.auction_id === auctionId));
        } catch (err) {
            console.error('Failed to load auction data:', err);
            toast.error('Failed to load auction data');
        } finally {
            setLoading(false);
        }
    };

    // ============================================================
    // Inline editing handlers
    // ============================================================
    const handleConditionChange = async (itemId: string, condition: string) => {
        try {
            await api.updateItemCondition(itemId, condition);
            setAuctionItems(prev => prev.map(i => i.id === itemId ? { ...i, condition } : i));
        } catch { toast.error('Failed to update condition'); }
    };

    const handleSourceChange = async (itemId: string, source: string) => {
        try {
            await api.updateItemSource(itemId, source);
            setAuctionItems(prev => prev.map(i => i.id === itemId ? { ...i, source } : i));
        } catch { toast.error('Failed to update source'); }
    };

    const handleBuybackerChange = async (itemId: string, buybackerId: string) => {
        try {
            await api.updateItemBuybacker(itemId, buybackerId);
            setAuctionItems(prev => prev.map(i => i.id === itemId ? { ...i, buybacker_id: buybackerId } : i));
        } catch { toast.error('Failed to update buy-backer'); }
    };

    const handleAddSupplier = async () => {
        if (!newSupplierName.trim()) return;
        try {
            await api.addSourceType(newSupplierName.trim());
            toast.success(`Supplier "${newSupplierName.trim()}" added successfully`);
            setNewSupplierName('');
            await loadReferenceData();
            // Automatically add this new supplier to vendorCosts so it shows up immediately
            setVendorCosts(prev => ({ ...prev, [newSupplierName.trim()]: 0.15 }));
        } catch (e) {
            toast.error('Failed to add supplier');
        }
    };

    const handleDeleteSupplier = async (name: string) => {
        if (!window.confirm(`Are you sure you want to delete supplier "${name}"? This will remove related costs.`)) return;
        try {
            await api.deleteSourceType(name);
            toast.success(`Supplier "${name}" deleted`);
            await loadReferenceData();
            setVendorCosts(prev => {
                const next = { ...prev };
                delete next[name];
                return next;
            });
        } catch (err) {
            toast.error('Failed to delete supplier');
        }
    };

    // ============================================================
    // Pricing recalculation
    // ============================================================
    const handleOpenPricingPanel = () => {
        const uniqueSources = Array.from(new Set([
            ...auctionItems.map(i => i.source).filter(s => Boolean(s) && s !== 'Unknown'),
            ...vendors.map(v => v.name)
        ])) as string[];
        const initialCosts: Record<string, number> = { ...vendorCosts };
        uniqueSources.forEach(source => {
            if (initialCosts[source] === undefined) {
                const v = vendors.find(vendor => source.toLowerCase().includes(vendor.name.toLowerCase())) 
                          || vendors.find(vendor => vendor.name === 'Amazon Bstock');
                initialCosts[source] = v ? v.cost_coefficient : 0.15;
            }
        });
        setVendorCosts(initialCosts);
        
        const initialMargins: Record<string, number> = { ...conditionMargins };
        conditionTypes.forEach(c => {
             if (initialMargins[c.label] === undefined) {
                  initialMargins[c.label] = 0.10;
             }
        });
        setConditionMargins(initialMargins);
        
        setShowPricingPanel(!showPricingPanel);
    };

    const handleRecalculatePrices = async () => {
        if (!auction) return;
        try {
            const count = await api.recalculatePrices(auction.id, vendorCosts, conditionMargins);
            toast.success(`Recalculated prices for ${count} items`);
            await loadAuctionData(auction.id);
            setShowPricingPanel(false);
        } catch (err) {
            toast.error('Failed to recalculate prices');
        }
    };

    // ============================================================
    // History (Repeaters)
    // ============================================================
    const handleShowHistory = async (normalizedTitle: string | undefined) => {
        if (!normalizedTitle) {
            toast.info('No normalized title for this item');
            return;
        }
        setHistoryItem(normalizedTitle);
        setHistoryLoading(true);
        try {
            const entries = await api.getItemHistory(normalizedTitle);
            setHistoryEntries(entries);
        } catch { setHistoryEntries([]); }
        finally { setHistoryLoading(false); }
    };

    // ============================================================
    // Finish & Export
    // ============================================================
    const handleFinish = async () => {
        if (!auction) return;
        const filePath = await api.selectFile([{ name: 'CSV Files', extensions: ['csv'] }]);
        if (!filePath) { toast.info('No file selected — auction not finished'); return; }

        setIsUpdatingStatus(true);
        try {
            const result = await api.finishAuction(auction.id, filePath as string);
            setAuction({ ...auction, status: 'Completed' as any });
            toast.success(`Auction finished! Reports generated: ${result.detail_report}, ${result.summary_report}`);
        } catch (error) {
            toast.error('Failed to finish auction: ' + String(error));
        } finally { setIsUpdatingStatus(false); }
    };

    const handleExportExcel = async () => {
        if (!auction || auctionItems.length === 0) return;
        try {
            const formattedData = auctionItems.map((item, idx) => {
                const r = idx + 2;
                const retail = Math.round(item.retail_price || 0);
                const cost_pct = (item.retail_price || 0) > 0 ? Math.round(((item.cost_price || 0) / (item.retail_price || 1)) * 100) : 0;
                return {
                    'Auction name': auction.name,
                    'LotNumber': item.lot_number || '',
                    'Quantity': 1,
                    'Title': item.raw_title || '',
                    'Vendor Code': item.source === 'Best Buy' ? 'ATXSUGAR' : '',
                    'Retail Price': retail,
                    'Source': item.source || '',
                    'Condition': item.condition || '',
                    'cost': (cost_pct / 100),
                    'cost price': { t: 'n', f: `ROUND(F${r}*I${r}, 2)` },
                    '% min pr (+10%)': { t: 'n', f: `ROUND(F${r}*0.10, 2)` },
                    'min price': { t: 'n', f: `CEILING(J${r}+K${r}, 1)` },
                    'Sale Order': item.sale_order || '',
                };
            });

            const worksheet = XLSX.utils.json_to_sheet(formattedData);
            const workbook = XLSX.utils.book_new();
            XLSX.utils.book_append_sheet(workbook, worksheet, "Sheet1");
            const excelBuffer = XLSX.write(workbook, { bookType: 'xlsx', type: 'array' });
            const defaultName = `${auction.name.replace(/\s+/g, '_')}_Manager_Report.xlsx`;
            const savePath = await api.saveFile(defaultName);
            if (!savePath) return;
            await api.saveBinaryFile(savePath as string, new Uint8Array(excelBuffer));
            toast.success(`Exported Excel to ${savePath}`);
        } catch (error) {
            toast.error('Failed to export Excel');
        }
    };

    // ============================================================
    // Loading / Not Found states
    // ============================================================
    if (loading) return <div className="p-8 text-center text-muted-foreground">Loading auction details...</div>;
    if (!auction) return <div className="p-8 text-center text-red-500">Auction not found</div>;

    // ============================================================
    // Derived data
    // ============================================================
    const totalRetail = auctionItems.reduce((acc, item) => acc + (item.retail_price || 0), 0);
    const estRevenue = auctionItems.reduce((acc, item) => acc + (item.min_price || 0), 0);

    const filteredItems = auctionItems.filter(item => {
        const matchesSearch = item.raw_title.toLowerCase().includes(searchQuery.toLowerCase()) ||
            item.lot_number?.toLowerCase().includes(searchQuery.toLowerCase());
        const matchesStatus = statusFilter === 'all' || item.current_status === statusFilter;
        const matchesVendor = vendorFilter === 'all' || item.source === vendorFilter;
        return matchesSearch && matchesStatus && matchesVendor;
    });

    const sortedItems = [...filteredItems].sort((a, b) => {
        if (!sortConfig) return 0;
        const { column, direction, pinnedValue } = sortConfig;

        // Pinned value logic (only for Source column for now as per request)
        if (column === 'source' && pinnedValue) {
            const aIsPinned = a.source === pinnedValue;
            const bIsPinned = b.source === pinnedValue;
            if (aIsPinned && !bIsPinned) return -1;
            if (!aIsPinned && bIsPinned) return 1;
        }

        let aVal: any;
        let bVal: any;
        switch (column) {
            case 'lot_number': return direction === 'asc' ? naturalSort(a.lot_number || '', b.lot_number || '') : naturalSort(b.lot_number || '', a.lot_number || '');
            case 'raw_title': aVal = a.raw_title.toLowerCase(); bVal = b.raw_title.toLowerCase(); break;
            case 'condition': aVal = (a.condition || '').toLowerCase(); bVal = (b.condition || '').toLowerCase(); break;
            case 'source': aVal = (a.source || '').toLowerCase(); bVal = (b.source || '').toLowerCase(); break;
            case 'retail_price': aVal = a.retail_price || 0; bVal = b.retail_price || 0; break;
            case 'cost_pct': aVal = a.retail_price ? (a.cost_price || 0) / a.retail_price : 0; bVal = b.retail_price ? (b.cost_price || 0) / b.retail_price : 0; break;
            case 'cost_price': aVal = a.cost_price || 0; bVal = b.cost_price || 0; break;
            case 'min_pr_10': aVal = a.retail_price ? a.retail_price * 0.10 : 0; bVal = b.retail_price ? b.retail_price * 0.10 : 0; break;
            case 'min_price': aVal = a.min_price || 0; bVal = b.min_price || 0; break;
            case 'buybacker': aVal = (a.buybacker_id || '').toLowerCase(); bVal = (b.buybacker_id || '').toLowerCase(); break;
            case 'current_status': aVal = a.current_status.toLowerCase(); bVal = b.current_status.toLowerCase(); break;
            case 'sale_order': aVal = a.sale_order || 9999; bVal = b.sale_order || 9999; break;
            default: return 0;
        }
        if (aVal < bVal) return direction === 'asc' ? -1 : 1;
        if (aVal > bVal) return direction === 'asc' ? 1 : -1;
        return 0;
    });

    const uniqueVendors = Array.from(new Set([
        ...auctionItems.map(i => i.source).filter(Boolean),
        ...vendors.map(v => v.name)
    ])) as string[];
    const pricingVendors = uniqueVendors.filter(v => v !== 'Unknown');

    const getStatusColor = (status: string) => {
        switch (status) {
            case 'Active': return 'bg-blue-500/15 text-blue-700 dark:text-blue-300';
            case 'Completed': return 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300';
            case 'Listed': return 'bg-amber-500/15 text-amber-700 dark:text-amber-300';
            case 'Sold': return 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300';
            case 'Buyback': return 'bg-red-500/15 text-red-700 dark:text-red-300';
            case 'FloorSale': return 'bg-orange-500/15 text-orange-700 dark:text-orange-300';
            case 'Unsold': return 'bg-gray-500/15 text-gray-700 dark:text-gray-300';
            default: return 'bg-gray-500/15 text-gray-700 dark:text-gray-300';
        }
    };

    const conditionOptions = conditionTypes.map(ct => ({ value: ct.label, label: ct.label }));
    const sourceOptions = [...vendors]
        .sort((a, b) => a.name.localeCompare(b.name))
        .map(v => ({ value: v.name, label: v.name }));
    const buybackerOptions = buybackers.map(bb => ({ value: bb.id, label: bb.name }));

    return (
        <div className="space-y-8 animate-fade-in pb-12">
            {/* Header Section */}
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                    <Button variant="ghost" size="icon" onClick={() => navigate('/auctions')}>
                        <ArrowLeft className="h-5 w-5" />
                    </Button>
                    <div>
                        <div className="flex items-center gap-3">
                            <h1 className="text-3xl font-bold tracking-tight">{auction.name}</h1>
                            <Badge variant="secondary" className={getStatusColor(auction.status)}>
                                {auction.status}
                            </Badge>
                        </div>
                    </div>
                </div>
                <div className="flex gap-2">
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={handleOpenPricingPanel}
                    >
                        <Calculator className="mr-2 h-4 w-4" />
                        Pricing
                    </Button>
                    {['Active', 'Completed'].includes(auction.status) && (
                        <Button
                            variant="default"
                            onClick={handleFinish}
                            disabled={isUpdatingStatus}
                            className="bg-emerald-600 hover:bg-emerald-700"
                        >
                            <Flag className="mr-2 h-4 w-4" />
                            {isUpdatingStatus ? 'Finishing...' : (auction.status === 'Completed' ? 'Regenerate Reports' : 'Finish')}
                        </Button>
                    )}
                    {['Active', 'Completed'].includes(auction.status) && (
                        <Button
                            variant="outline"
                            onClick={handleExportExcel}
                            disabled={auctionItems.length === 0}
                            className="text-emerald-600 border-emerald-500 hover:bg-emerald-50 hover:text-emerald-700 w-9 px-0"
                        >
                            <FileSpreadsheet className="h-4 w-4" />
                        </Button>
                    )}
                </div>
            </div>

            {/* Pricing Calculator Panel */}
            {showPricingPanel && (
                <Card className="border-amber-200 bg-amber-50/30 dark:bg-amber-950/20 animate-fade-in">
                    <CardHeader className="pb-3">
                        <CardTitle className="text-base flex items-center gap-2">
                            <Calculator className="h-4 w-4" />
                            Dynamic Pricing Calculator
                        </CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
                            {/* Cost Settings */}
                            <div>
                                <h4 className="text-sm font-semibold mb-3">Supplier Cost Percentage</h4>
                                <div className="space-y-2">
                                    {pricingVendors.length === 0 ? (
                                        <p className="text-xs text-muted-foreground">No suppliers found in this auction.</p>
                                    ) : (
                                        pricingVendors.map(vendor => (
                                            <div key={vendor} className="flex items-center justify-between bg-background border rounded px-3 py-2 group">
                                                <div className="flex items-center gap-2">
                                                    <span className="text-sm font-medium">{vendor}</span>
                                                    <Button
                                                        variant="ghost"
                                                        size="icon"
                                                        className="h-5 w-5 text-muted-foreground hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity"
                                                        onClick={() => handleDeleteSupplier(vendor)}
                                                    >
                                                        <Trash2 className="h-3 w-3" />
                                                    </Button>
                                                </div>
                                                <div className="flex items-center gap-1">
                                                    <Input
                                                        type="number"
                                                        step="1"
                                                        className="w-16 h-7 text-xs text-center"
                                                        value={vendorCosts[vendor] !== undefined ? Math.round(vendorCosts[vendor] * 100) : 15}
                                                        onChange={(e) => {
                                                            const val = parseFloat(e.target.value);
                                                            if (!isNaN(val)) {
                                                                setVendorCosts(prev => ({ ...prev, [vendor]: val / 100 }));
                                                            }
                                                        }}
                                                    />
                                                    <span className="text-sm">%</span>
                                                </div>
                                            </div>
                                        ))
                                    )}
                                </div>
                                <div className="mt-4 border border-dashed rounded-lg p-2">
                                    <div className="flex gap-2 w-full">
                                        <Input
                                            value={newSupplierName}
                                            onChange={(e) => setNewSupplierName(e.target.value)}
                                            placeholder="New supplier name"
                                            className="h-9 text-sm w-full bg-background"
                                            onKeyDown={(e) => {
                                                if (e.key === 'Enter') handleAddSupplier();
                                            }}
                                        />
                                        <Button
                                            size="sm"
                                            onClick={handleAddSupplier}
                                            className="h-9 shrink-0 bg-muted-foreground/80 hover:bg-muted-foreground text-primary-foreground"
                                        >
                                            <PlusCircle className="mr-2 h-4 w-4" />
                                            Add
                                        </Button>
                                    </div>
                                </div>
                            </div>
                            
                            
                            {/* Margin Settings */}
                            <div>
                                <h4 className="text-sm font-semibold mb-3">Condition Margin Percentage</h4>
                                <div className="space-y-2">
                                    {conditionTypes.map(ct => (
                                        <div key={ct.id} className="flex items-center justify-between bg-background border rounded px-3 py-2">
                                            <span className="text-sm font-medium">{ct.label}</span>
                                            <div className="flex items-center gap-1">
                                                <Input
                                                    type="number"
                                                    step="1"
                                                    className="w-16 h-7 text-xs text-center"
                                                    value={conditionMargins[ct.label] !== undefined ? Math.round(conditionMargins[ct.label] * 100) : 10}
                                                    onChange={(e) => {
                                                        const val = parseFloat(e.target.value);
                                                        if (!isNaN(val)) {
                                                            setConditionMargins(prev => ({ ...prev, [ct.label]: val / 100 }));
                                                        }
                                                    }}
                                                />
                                                <span className="text-sm">%</span>
                                            </div>
                                        </div>
                                    ))}
                                </div>
                            </div>
                        </div>
                        <div className="mt-6 flex flex-col sm:flex-row items-center justify-between border-t border-amber-200/50 pt-4">
                            <div className="text-sm text-balance max-w-lg space-y-1">
                                <p className="font-semibold text-amber-900 dark:text-amber-200 flex items-center gap-2">
                                    <Calculator className="h-4 w-4" /> Calculations
                                </p>
                                <ul className="list-disc list-inside text-xs text-muted-foreground ml-1">
                                    <li><span className="font-medium text-foreground">Cost Price</span> = <span className="font-mono bg-muted px-1 rounded">Retail Price</span> &times; <span className="font-mono bg-amber-500/20 px-1 rounded text-amber-900 dark:text-amber-200">[Supplier Cost %]</span></li>
                                    <li><span className="font-medium text-foreground">Min Price</span> = <span className="font-medium text-foreground">Cost Price</span> + (<span className="font-mono bg-muted px-1 rounded">Retail Price</span> &times; <span className="font-mono bg-amber-500/20 px-1 rounded text-amber-900 dark:text-amber-200">[Condition Margin %]</span>)</li>
                                </ul>
                            </div>
                            <Button
                                className="bg-amber-600 hover:bg-amber-700 mt-4 sm:mt-0 shadow-sm"
                                onClick={handleRecalculatePrices}
                            >
                                <RefreshCw className="mr-2 h-4 w-4" />
                                Recalculate {auctionItems.length} Items
                            </Button>
                        </div>
                    </CardContent>
                </Card>
            )}

            {/* KPI Section */}
            <div className="grid gap-4 md:grid-cols-3">
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium text-muted-foreground">Total Lots</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{auctionItems.length}</div>
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium text-muted-foreground">Total Retail</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold text-emerald-600">{formatCurrencyWhole(totalRetail)}</div>
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium text-muted-foreground">Est. Revenue</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold text-blue-600">{formatCurrencyWhole(estRevenue)}</div>
                    </CardContent>
                </Card>
            </div>

            {/* Item History Popup */}
            {historyItem && (
                <Card className="border-indigo-200 bg-indigo-50/30 dark:bg-indigo-950/20 animate-fade-in">
                    <CardHeader className="pb-2">
                        <div className="flex items-center justify-between">
                            <CardTitle className="text-sm flex items-center gap-2">
                                <History className="h-4 w-4" />
                                Bid History: {historyItem.substring(0, 60)}...
                            </CardTitle>
                            <Button variant="ghost" size="icon" className="h-6 w-6" onClick={() => setHistoryItem(null)}>
                                <X className="h-3 w-3" />
                            </Button>
                        </div>
                    </CardHeader>
                    <CardContent>
                        {historyLoading ? (
                            <p className="text-xs text-muted-foreground">Loading history...</p>
                        ) : historyEntries.length === 0 ? (
                            <p className="text-xs text-muted-foreground">No previous sales found for this item.</p>
                        ) : (
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHead className="text-xs">Auction</TableHead>
                                        <TableHead className="text-xs">Lot</TableHead>
                                        <TableHead className="text-xs text-right">Bid</TableHead>
                                        <TableHead className="text-xs">Bidder</TableHead>
                                        <TableHead className="text-xs">Date</TableHead>
                                        <TableHead className="text-xs">Type</TableHead>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {historyEntries.map((entry, idx) => (
                                        <TableRow key={idx}>
                                            <TableCell className="text-xs">{entry.auction_name}</TableCell>
                                            <TableCell className="text-xs">{entry.lot_number || '-'}</TableCell>
                                            <TableCell className="text-xs text-right font-medium">{formatCurrencyWhole(entry.high_bid)}</TableCell>
                                            <TableCell className="text-xs">{entry.bidder_name}</TableCell>
                                            <TableCell className="text-xs">{entry.sale_date?.split('T')[0]}</TableCell>
                                            <TableCell><Badge variant="secondary" className={entry.is_buyback ? 'bg-red-100 text-red-700' : 'bg-emerald-100 text-emerald-700'}>{entry.is_buyback ? 'Buyback' : 'Sold'}</Badge></TableCell>
                                        </TableRow>
                                    ))}
                                </TableBody>
                            </Table>
                        )}
                    </CardContent>
                </Card>
            )}

            {/* Auction Items Section */}
            <Card>
                <CardHeader>
                    <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                        <CardTitle>Auction Items</CardTitle>
                        <div className="flex flex-wrap items-center gap-2">
                            <div className="relative w-full sm:w-64">
                                <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                                <Input
                                    placeholder="Search lot, title..."
                                    className="pl-9 bg-background/80"
                                    value={searchQuery}
                                    onChange={(e) => setSearchQuery(e.target.value)}
                                />
                            </div>

                            <select
                                className="flex h-10 items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
                                value={statusFilter}
                                onChange={(e) => setStatusFilter(e.target.value)}
                            >
                                <option value="all">All Statuses</option>
                                <option value="Listed">Listed</option>
                                <option value="Sold">Sold</option>
                                <option value="Buyback">Buyback</option>
                                <option value="FloorSale">Floor Sale</option>
                                <option value="Unsold">Unsold</option>
                                <option value="Scrap">Scrap</option>
                            </select>
                            {uniqueVendors.length > 0 && (
                                <select
                                    className="flex h-10 items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
                                    value={vendorFilter}
                                    onChange={(e) => setVendorFilter(e.target.value)}
                                >
                                    <option value="all">All Vendors</option>
                                    <option value="Unknown">Unknown / Unspecified</option>
                                    {pricingVendors.map(v => (
                                        <option key={v} value={v}>{v}</option>
                                    ))}
                                </select>
                            )}
                        </div>
                    </div>
                </CardHeader>
                <CardContent>
                    <div className="overflow-x-auto">
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <SortableTableHead column="lot_number" label="Lot #" sortConfig={sortConfig} onSort={handleSort} className="w-[80px]" isText />
                                    <SortableTableHead column="raw_title" label="Title" sortConfig={sortConfig} onSort={handleSort} isText />
                                    <SortableTableHead column="condition" label="Condition" sortConfig={sortConfig} onSort={handleSort} className="w-[140px]" isText />
                                    <SortableTableHead
                                        column="source"
                                        label="Source"
                                        sortConfig={sortConfig}
                                        onSort={handleSort}
                                        className="w-[120px]"
                                        isText
                                        extraItems={pricingVendors.map(v => ({
                                            label: v,
                                            onClick: () => handleSort('source', sortConfig?.direction || 'asc', v),
                                            isActive: sortConfig?.column === 'source' && sortConfig?.pinnedValue === v
                                        }))}
                                    />
                                    <SortableTableHead column="retail_price" label="Retail" sortConfig={sortConfig} onSort={handleSort} className="text-right" />
                                    <SortableTableHead column="cost_pct" label="Cost %" sortConfig={sortConfig} onSort={handleSort} className="text-right" />
                                    <SortableTableHead column="cost_price" label="Cost Price" sortConfig={sortConfig} onSort={handleSort} className="text-right" />
                                    <SortableTableHead column="min_pr_10" label="Min Pr (+10%)" sortConfig={sortConfig} onSort={handleSort} className="text-right" />
                                    <SortableTableHead column="min_price" label="Min Price" sortConfig={sortConfig} onSort={handleSort} className="text-right" />
                                    {auction.status === 'Completed' && (
                                        <SortableTableHead column="buybacker" label="Buy-backer" sortConfig={sortConfig} onSort={handleSort} className="w-[130px]" isText />
                                    )}
                                    <SortableTableHead column="current_status" label="Status" sortConfig={sortConfig} onSort={handleSort} isText />
                                    <TableHead className="w-[60px]"></TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {sortedItems.length === 0 ? (
                                    <TableRow>
                                        <TableCell colSpan={11} className="h-24 text-center text-muted-foreground">
                                            No items found
                                        </TableCell>
                                    </TableRow>
                                ) : (
                                    sortedItems.map((item) => (
                                        <TableRow key={item.id} className="group">
                                            <TableCell className="font-medium font-mono text-xs">{item.lot_number || '-'}</TableCell>
                                            <TableCell className="max-w-[250px]">
                                                <div className="truncate text-sm font-medium">{item.raw_title}</div>
                                                <div className="text-[10px] text-muted-foreground truncate">{item.vendor_code || ''}</div>
                                            </TableCell>
                                            {/* Inline Condition Dropdown */}
                                            <TableCell>
                                                <InlineSelect
                                                    value={item.condition || ''}
                                                    options={conditionOptions}
                                                    onChange={(val) => handleConditionChange(item.id, val)}
                                                    placeholder="— Set —"
                                                />
                                            </TableCell>
                                            {/* Inline Source Dropdown */}
                                            <TableCell>
                                                <div className="flex items-center gap-1">
                                                    {(!item.source || item.source === 'Unknown') && (
                                                        <span title="Supplier not specified" className="flex shrink-0">
                                                            <AlertCircle className="h-4 w-4 text-red-500" />
                                                        </span>
                                                    )}
                                                    <InlineSelect
                                                        className={(!item.source || item.source === 'Unknown') ? 'text-red-500 font-medium' : ''}
                                                        value={item.source || ''}
                                                        options={sourceOptions}
                                                        onChange={(val) => handleSourceChange(item.id, val)}
                                                        placeholder="— Set —"
                                                    />
                                                </div>
                                            </TableCell>
                                            <TableCell className="text-right font-medium">{formatCurrencyWhole(item.retail_price || 0)}</TableCell>
                                            <TableCell className="text-right text-muted-foreground text-xs">
                                                {item.retail_price ? Math.round(((item.cost_price || 0) / item.retail_price) * 100) + '%' : '0%'}
                                            </TableCell>
                                            <TableCell className="text-right">{formatCurrencyWhole(item.cost_price || 0)}</TableCell>
                                            <TableCell className="text-right text-muted-foreground text-xs">
                                                {formatCurrencyWhole(Math.round((item.retail_price || 0) * 0.10))}
                                            </TableCell>
                                            <TableCell className="text-right font-semibold text-emerald-600 dark:text-emerald-400">
                                                {formatCurrencyWhole(Math.ceil(item.min_price || 0))}
                                            </TableCell>
                                            {/* Inline Buy-backer Dropdown */}
                                            {auction.status === 'Completed' && (
                                                <TableCell>
                                                    <InlineSelect
                                                        value={item.buybacker_id || ''}
                                                        options={buybackerOptions}
                                                        onChange={(val) => handleBuybackerChange(item.id, val)}
                                                        placeholder="—"
                                                    />
                                                </TableCell>
                                            )}
                                            <TableCell>
                                                <Badge variant="secondary" className={getStatusColor(item.current_status)}>
                                                    {item.current_status}
                                                </Badge>
                                            </TableCell>
                                            <TableCell>
                                                <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                                                    <Button
                                                        variant="ghost" size="icon" className="h-7 w-7 text-indigo-500"
                                                        title="View bid history"
                                                        onClick={() => handleShowHistory(item.normalized_title)}
                                                    >
                                                        <History className="h-3.5 w-3.5" />
                                                    </Button>
                                                    {auction.status !== 'Completed' && (
                                                        <Button
                                                            variant="ghost" size="icon"
                                                            className="h-7 w-7 text-muted-foreground hover:text-red-500"
                                                            onClick={async () => {
                                                                try {
                                                                    await api.unassignItem(item.id);
                                                                    toast.success('Item removed from auction');
                                                                    if (auction) await loadAuctionData(auction.id);
                                                                } catch { toast.error('Failed to remove item'); }
                                                            }}
                                                        >
                                                            <X className="h-3.5 w-3.5" />
                                                        </Button>
                                                    )}
                                                </div>
                                            </TableCell>
                                        </TableRow>
                                    ))
                                )}
                            </TableBody>
                        </Table>
                    </div>
                    <div className="mt-4 text-xs text-muted-foreground text-center">
                        Showing {sortedItems.length} of {auctionItems.length} items
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
