import { useEffect, useState, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { api } from '@/lib/api';
import type { Auction, InventoryItem, ConditionType, Buybacker, ItemHistoryEntry, Vendor } from '@/types';
import { ArrowLeft, X, Search, Flag, FileSpreadsheet, Calculator, History, RefreshCw, PlusCircle, AlertCircle, Trash2, CheckSquare } from 'lucide-react';
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
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogDescription } from '@/components/ui/dialog';
import { formatCurrencyWhole, formatNumber, naturalSort } from '@/lib/utils';
import { buildManagerReportFileName } from '@/lib/auctionNaming';
import { buildManagerReportPreviewRows, buildManagerReportWorkbook, getHistoryHeadersForAuction } from '@/lib/managerReport';
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
    const [conditionMarginsBySupplier, setConditionMarginsBySupplier] = useState<Record<string, Record<string, number>>>({});
    const [showPricingPanel, setShowPricingPanel] = useState(false);
    const [newSupplierName, setNewSupplierName] = useState('');

    // History popover
    const [historyItem, setHistoryItem] = useState<string | null>(null);
    const [historyEntries, setHistoryEntries] = useState<ItemHistoryEntry[]>([]);
    const [historyLoading, setHistoryLoading] = useState(false);
    const [repeaterStatsByTitle, setRepeaterStatsByTitle] = useState<Record<string, Record<string, number>>>({});

    // Relist from inventory dialog
    const [showRelistDialog, setShowRelistDialog] = useState(false);
    const [relistCandidates, setRelistCandidates] = useState<InventoryItem[]>([]);
    const [relistLoading, setRelistLoading] = useState(false);
    const [relistSearch, setRelistSearch] = useState('');
    const [selectedRelistIds, setSelectedRelistIds] = useState<string[]>([]);

    // Filters
    const [searchQuery, setSearchQuery] = useState('');

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

    useEffect(() => {
        const loadRepeaterStats = async () => {
            if (!auction || auctionItems.length === 0) {
                setRepeaterStatsByTitle({});
                return;
            }
            const headers = getHistoryHeadersForAuction(auction.name);
            const titles = Array.from(
                new Set(
                    auctionItems
                        .map((item) => item.normalized_title?.trim())
                        .filter((v): v is string => Boolean(v)),
                ),
            );
            if (titles.length === 0) {
                setRepeaterStatsByTitle({});
                return;
            }
            try {
                const stats = await api.getItemRepeaterStats(titles, headers);
                setRepeaterStatsByTitle(stats || {});
            } catch (err) {
                console.error('Failed to load repeater stats:', err);
                setRepeaterStatsByTitle({});
            }
        };
        loadRepeaterStats();
    }, [auction, auctionItems]);

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

    const loadRelistCandidates = async (auctionId: string) => {
        setRelistLoading(true);
        try {
            const candidates = await api.getRelistableInventoryItems(auctionId);
            setRelistCandidates(candidates);
        } catch (err) {
            console.error('Failed to load relist candidates:', err);
            toast.error('Failed to load inventory items for relist');
            setRelistCandidates([]);
        } finally {
            setRelistLoading(false);
        }
    };

    const openRelistDialog = async () => {
        if (!auction) return;
        setShowRelistDialog(true);
        setRelistSearch('');
        setSelectedRelistIds([]);
        await loadRelistCandidates(auction.id);
    };

    const toggleRelistItem = (itemId: string) => {
        setSelectedRelistIds((prev) => (
            prev.includes(itemId) ? prev.filter((id) => id !== itemId) : [...prev, itemId]
        ));
    };

    const assignSelectedItemsToAuction = async () => {
        if (!auction || selectedRelistIds.length === 0) return;
        try {
            const assigned = await api.assignItemsToAuction(auction.id, selectedRelistIds);
            toast.success(`Added ${assigned} item(s) to auction`);
            setShowRelistDialog(false);
            await loadAuctionData(auction.id);
        } catch (err) {
            console.error('Failed to assign items to auction:', err);
            toast.error('Failed to add selected items to auction');
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
        const supplierName = newSupplierName.trim();
        if (!supplierName) return;
        try {
            await api.addSourceType(supplierName);
            toast.success(`Supplier "${supplierName}" added successfully`);
            setNewSupplierName('');
            await loadReferenceData();
            // Automatically add this new supplier to vendorCosts so it shows up immediately
            setVendorCosts(prev => ({ ...prev, [supplierName]: 0.15 }));
            setConditionMarginsBySupplier(prev => {
                const next = { ...prev };
                if (next[supplierName] === undefined) {
                    const defaultMargins: Record<string, number> = {};
                    conditionTypes.forEach(ct => {
                        defaultMargins[ct.label] = 0.10;
                    });
                    next[supplierName] = defaultMargins;
                }
                return next;
            });
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
            setConditionMarginsBySupplier(prev => {
                const next = { ...prev };
                delete next[name];
                return next;
            });
            setAuctionItems(prev => prev.map(item => (
                item.source === name ? { ...item, source: 'Unknown' } : item
            )));
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
        
        const initialMarginsBySupplier: Record<string, Record<string, number>> = { ...conditionMarginsBySupplier };
        uniqueSources.forEach(source => {
            const sourceMargins: Record<string, number> = { ...(initialMarginsBySupplier[source] || {}) };
            conditionTypes.forEach(c => {
                if (sourceMargins[c.label] === undefined) {
                    sourceMargins[c.label] = 0.10;
                }
            });
            initialMarginsBySupplier[source] = sourceMargins;
        });
        setConditionMarginsBySupplier(initialMarginsBySupplier);
        
        setShowPricingPanel(true);
    };

    const handleRecalculatePrices = async () => {
        if (!auction) return;
        try {
            const count = await api.recalculatePrices(auction.id, vendorCosts, conditionMarginsBySupplier);
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
        if (!filePath) { toast.info('No file selected - auction not finished'); return; }

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
            const workbook = buildManagerReportWorkbook({
                items: auctionItems,
                auctionName: auction.name,
                vendorCosts,
                conditionMarginsBySupplier,
                historyHeaders: getHistoryHeadersForAuction(auction.name),
                repeaterStatsByTitle,
            });
            const excelBuffer = XLSX.write(workbook, { bookType: 'xlsx', type: 'array' });
            const defaultName = buildManagerReportFileName(auction.name);
            const savePath = await api.saveFile(defaultName);
            if (!savePath) return;
            await api.saveBinaryFile(savePath as string, new Uint8Array(excelBuffer));
            toast.success(`Exported Excel to ${savePath}`);
        } catch (error) {
            toast.error('Failed to export Excel');
        }
    };

    const uniqueVendors = Array.from(new Set([
        ...auctionItems.map(i => i.source).filter(Boolean),
        ...vendors.map(v => v.name)
    ])) as string[];
    const pricingVendors = uniqueVendors.filter(v => v !== 'Unknown');

    const handleResetConditionMargins = () => {
        if (!window.confirm('Reset all condition margins to 10% for all suppliers?')) return;
        setConditionMarginsBySupplier(prev => {
            const next = { ...prev };
            pricingVendors.forEach(vendor => {
                const supplierMargins = { ...(next[vendor] || {}) };
                conditionTypes.forEach(ct => {
                    supplierMargins[ct.label] = 0.10;
                });
                next[vendor] = supplierMargins;
            });
            return next;
        });
        toast.success('Condition margins reset to 10%');
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
    const historyHeaders = getHistoryHeadersForAuction(auction.name);
    const relistFilteredItems = relistCandidates.filter((item) => {
        const q = relistSearch.trim().toLowerCase();
        if (!q) return true;
        return (
            item.raw_title.toLowerCase().includes(q) ||
            (item.lot_number || '').toLowerCase().includes(q) ||
            (item.source || '').toLowerCase().includes(q)
        );
    });
    const selectedRelistSet = new Set(selectedRelistIds);
    const selectedVisibleCount = relistFilteredItems.reduce((count, item) => (
        count + (selectedRelistSet.has(item.id) ? 1 : 0)
    ), 0);

    const reportPreviewRows = buildManagerReportPreviewRows({
        items: auctionItems,
        auctionName: auction.name,
        vendorCosts,
        conditionMarginsBySupplier,
        historyHeaders,
        repeaterStatsByTitle,
    });

    const filteredRows = reportPreviewRows.filter(row => {
        return row.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
            row.lotNumber.toLowerCase().includes(searchQuery.toLowerCase());
    });

    const sortedRows = [...filteredRows].sort((a, b) => {
        if (!sortConfig) return 0;
        const { column, direction, pinnedValue } = sortConfig;

        // Pinned value logic for Source column.
        if (column === 'source' && pinnedValue) {
            const isUnspecified = (s: string | undefined | null) => !s || s === 'Unknown';
            const aIsPinned = pinnedValue === 'Unknown' ? isUnspecified(a.source) : a.source === pinnedValue;
            const bIsPinned = pinnedValue === 'Unknown' ? isUnspecified(b.source) : b.source === pinnedValue;
            if (aIsPinned && !bIsPinned) return -1;
            if (!aIsPinned && bIsPinned) return 1;
        }

        let aVal: string | number = 0;
        let bVal: string | number = 0;
        switch (column) {
            case 'lot_number':
                return direction === 'asc'
                    ? naturalSort(a.lotNumber, b.lotNumber)
                    : naturalSort(b.lotNumber, a.lotNumber);
            case 'sale_order':
                aVal = a.saleOrder === '' ? 9999 : Number(a.saleOrder);
                bVal = b.saleOrder === '' ? 9999 : Number(b.saleOrder);
                break;
            case 'title':
                aVal = a.title.toLowerCase();
                bVal = b.title.toLowerCase();
                break;
            case 'retail_price':
                aVal = a.retailPrice;
                bVal = b.retailPrice;
                break;
            case 'source':
                aVal = a.source.toLowerCase();
                bVal = b.source.toLowerCase();
                break;
            case 'condition':
                aVal = a.condition.toLowerCase();
                bVal = b.condition.toLowerCase();
                break;
            case 'cost_pct':
                aVal = a.costPct;
                bVal = b.costPct;
                break;
            case 'min_price_pct':
                aVal = a.minPricePct;
                bVal = b.minPricePct;
                break;
            case 'cost_price':
                aVal = a.costPrice;
                bVal = b.costPrice;
                break;
            case 'min_price_half':
                aVal = a.minPriceHalf;
                bVal = b.minPriceHalf;
                break;
            case 'min_price_one':
                aVal = a.minPriceOne;
                bVal = b.minPriceOne;
                break;
            case 'repeat':
                aVal = a.repeat;
                bVal = b.repeat;
                break;
            case 'buybacker':
                aVal = (a.item.buybacker_id || '').toLowerCase();
                bVal = (b.item.buybacker_id || '').toLowerCase();
                break;
            case 'current_status':
                aVal = a.item.current_status.toLowerCase();
                bVal = b.item.current_status.toLowerCase();
                break;
            default:
                break;
        }
        if (historyHeaders.includes(column)) {
            aVal = a.historyBySeason[column] ?? 0;
            bVal = b.historyBySeason[column] ?? 0;
        }
        if (aVal < bVal) return direction === 'asc' ? -1 : 1;
        if (aVal > bVal) return direction === 'asc' ? 1 : -1;
        return 0;
    });
    const tableColumnCount = 14 + historyHeaders.length + (auction.status === 'Completed' ? 1 : 0);

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

            <Dialog open={showPricingPanel} onOpenChange={setShowPricingPanel}>
                <DialogContent className="max-w-6xl max-h-[90vh] overflow-y-auto">
                    <DialogHeader>
                        <DialogTitle className="flex items-center gap-2">
                            <Calculator className="h-4 w-4" />
                            Dynamic Pricing Calculator
                        </DialogTitle>
                        <DialogDescription>
                            Configure supplier costs and condition margins, then recalculate minimum prices.
                        </DialogDescription>
                    </DialogHeader>

                    <div className="space-y-8">
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

                        <div>
                            <div className="flex items-center justify-between mb-3 gap-3">
                                <h4 className="text-sm font-semibold">Condition Margin Matrix (%)</h4>
                                <Button
                                    type="button"
                                    size="sm"
                                    variant="outline"
                                    className="h-7 px-3 text-xs"
                                    onClick={handleResetConditionMargins}
                                >
                                    Reset to 10%
                                </Button>
                            </div>
                            {pricingVendors.length === 0 ? (
                                <p className="text-xs text-muted-foreground">No suppliers available for margin setup.</p>
                            ) : conditionTypes.length === 0 ? (
                                <p className="text-xs text-muted-foreground">No condition types configured.</p>
                            ) : (
                                <div className="overflow-x-auto rounded-lg border bg-background">
                                    <table className="min-w-full text-xs">
                                        <thead className="bg-muted/40">
                                            <tr className="border-b">
                                                <th className="text-left font-semibold px-3 py-2 min-w-[220px] border-r border-border/60">Condition</th>
                                                {pricingVendors.map(vendor => (
                                                    <th key={vendor} className="text-center font-semibold px-2 py-2 min-w-[120px] border-r border-border/60 last:border-r-0">
                                                        {vendor}
                                                    </th>
                                                ))}
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {conditionTypes.map((ct, idx) => (
                                                <tr key={ct.id} className={`border-b ${idx % 2 === 1 ? 'bg-muted/20' : ''}`}>
                                                    <td className="px-3 py-2 text-sm font-medium border-r border-border/60">{ct.label}</td>
                                                    {pricingVendors.map(vendor => (
                                                        <td key={`${ct.id}-${vendor}`} className="px-2 py-1.5 text-center border-r border-border/60 last:border-r-0">
                                                            <div className="inline-flex items-center gap-1">
                                                                <Input
                                                                    type="number"
                                                                    step="1"
                                                                    className="w-16 h-7 text-xs text-center"
                                                                    value={conditionMarginsBySupplier[vendor]?.[ct.label] !== undefined
                                                                        ? Math.round(conditionMarginsBySupplier[vendor][ct.label] * 100)
                                                                        : 10}
                                                                    onChange={(e) => {
                                                                        const val = parseFloat(e.target.value);
                                                                        if (!isNaN(val)) {
                                                                            setConditionMarginsBySupplier(prev => ({
                                                                                ...prev,
                                                                                [vendor]: {
                                                                                    ...(prev[vendor] || {}),
                                                                                    [ct.label]: val / 100
                                                                                }
                                                                            }));
                                                                        }
                                                                    }}
                                                                />
                                                                <span className="text-xs text-muted-foreground">%</span>
                                                            </div>
                                                        </td>
                                                    ))}
                                                </tr>
                                            ))}
                                        </tbody>
                                    </table>
                                </div>
                            )}
                            <p className="text-[11px] text-muted-foreground mt-2">
                                Each cell defines margin for a specific supplier/condition pair.
                            </p>
                        </div>

                        <div className="border rounded-lg p-3 bg-muted/20">
                            <p className="font-semibold text-sm flex items-center gap-2">
                                <Calculator className="h-4 w-4" />
                                Calculations
                            </p>
                            <ul className="list-disc list-inside text-xs text-muted-foreground mt-1">
                                <li><span className="font-medium text-foreground">Cost Price</span> = <span className="font-mono bg-muted px-1 rounded">Retail Price</span> &times; <span className="font-mono bg-muted px-1 rounded">[Supplier Cost %]</span></li>
                                <li><span className="font-medium text-foreground">Min Price</span> = <span className="font-medium text-foreground">Cost Price</span> + (<span className="font-mono bg-muted px-1 rounded">Retail Price</span> &times; <span className="font-mono bg-muted px-1 rounded">[Condition Margin %]</span>)</li>
                            </ul>
                        </div>
                    </div>

                    <DialogFooter>
                        <div className="flex w-full items-center justify-end gap-2">
                            <Button type="button" variant="outline" onClick={() => setShowPricingPanel(false)}>
                                Cancel
                            </Button>
                            <Button onClick={handleRecalculatePrices}>
                                <RefreshCw className="mr-2 h-4 w-4" />
                                Recalculate {auctionItems.length} Items
                            </Button>
                        </div>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            <Dialog open={showRelistDialog} onOpenChange={setShowRelistDialog}>
                <DialogContent className="max-w-5xl">
                    <DialogHeader>
                        <DialogTitle>Add Items From Inventory</DialogTitle>
                        <DialogDescription>
                            Select unsold or buyback inventory items and add them to this active auction.
                        </DialogDescription>
                    </DialogHeader>

                    <div className="space-y-3">
                        <div className="flex flex-col sm:flex-row gap-2 sm:items-center">
                            <Input
                                placeholder="Search by lot, title, source..."
                                value={relistSearch}
                                onChange={(e) => setRelistSearch(e.target.value)}
                            />
                            <div className="flex gap-2">
                                <Button
                                    type="button"
                                    variant="outline"
                                    size="sm"
                                    onClick={() => {
                                        const visibleIds = relistFilteredItems.map((item) => item.id);
                                        setSelectedRelistIds(Array.from(new Set([...selectedRelistIds, ...visibleIds])));
                                    }}
                                    disabled={relistFilteredItems.length === 0}
                                >
                                    Select All
                                </Button>
                                <Button
                                    type="button"
                                    variant="outline"
                                    size="sm"
                                    onClick={() => {
                                        const visibleSet = new Set(relistFilteredItems.map((item) => item.id));
                                        setSelectedRelistIds((prev) => prev.filter((id) => !visibleSet.has(id)));
                                    }}
                                    disabled={selectedVisibleCount === 0}
                                >
                                    Clear Visible
                                </Button>
                            </div>
                        </div>

                        <div className="max-h-[420px] overflow-auto rounded-lg border">
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHead className="w-[60px] text-center">Pick</TableHead>
                                        <TableHead className="w-[110px]">Lot</TableHead>
                                        <TableHead>Title</TableHead>
                                        <TableHead className="w-[130px]">Status</TableHead>
                                        <TableHead className="w-[130px]">Source</TableHead>
                                        <TableHead className="w-[120px] text-right">Min Price</TableHead>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {relistLoading ? (
                                        <TableRow>
                                            <TableCell colSpan={6} className="h-20 text-center text-muted-foreground">
                                                Loading inventory...
                                            </TableCell>
                                        </TableRow>
                                    ) : relistFilteredItems.length === 0 ? (
                                        <TableRow>
                                            <TableCell colSpan={6} className="h-20 text-center text-muted-foreground">
                                                No relistable items found
                                            </TableCell>
                                        </TableRow>
                                    ) : (
                                        relistFilteredItems.map((item) => {
                                            const isSelected = selectedRelistSet.has(item.id);
                                            return (
                                                <TableRow key={item.id}>
                                                    <TableCell className="text-center">
                                                        <input
                                                            type="checkbox"
                                                            checked={isSelected}
                                                            onChange={() => toggleRelistItem(item.id)}
                                                        />
                                                    </TableCell>
                                                    <TableCell className="font-mono">{item.lot_number || '-'}</TableCell>
                                                    <TableCell className="max-w-[520px]">
                                                        <div className="truncate">{item.raw_title}</div>
                                                    </TableCell>
                                                    <TableCell>
                                                        <Badge variant="secondary" className={getStatusColor(item.current_status)}>
                                                            {item.current_status}
                                                        </Badge>
                                                    </TableCell>
                                                    <TableCell>{item.source || '-'}</TableCell>
                                                    <TableCell className="text-right font-mono">
                                                        {formatNumber(Math.round(item.min_price || 0))}
                                                    </TableCell>
                                                </TableRow>
                                            );
                                        })
                                    )}
                                </TableBody>
                            </Table>
                        </div>
                    </div>

                    <DialogFooter>
                        <div className="flex w-full items-center justify-between">
                            <div className="text-xs text-muted-foreground">
                                Selected: {selectedRelistIds.length}
                            </div>
                            <div className="flex gap-2">
                                <Button type="button" variant="outline" onClick={() => setShowRelistDialog(false)}>
                                    Cancel
                                </Button>
                                <Button
                                    type="button"
                                    onClick={assignSelectedItemsToAuction}
                                    disabled={selectedRelistIds.length === 0}
                                >
                                    Add Selected
                                </Button>
                            </div>
                        </div>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

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
            <Card className="border-emerald-200/70 shadow-sm">
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
                            <Button
                                variant="outline"
                                size="sm"
                                onClick={handleOpenPricingPanel}
                            >
                                <Calculator className="mr-2 h-4 w-4" />
                                Pricing
                            </Button>
                            {auction.status === 'Active' && (
                                <Button
                                    variant="outline"
                                    size="sm"
                                    onClick={openRelistDialog}
                                >
                                    <CheckSquare className="mr-2 h-4 w-4" />
                                    Add From Inventory
                                </Button>
                            )}
                        </div>
                    </div>
                </CardHeader>
                <CardContent>
                    <div className="overflow-x-auto rounded-xl border border-emerald-200/80 bg-gradient-to-b from-emerald-50/30 to-background shadow-inner">
                        <Table className="min-w-[2500px] text-xs border-separate border-spacing-0">
                            <TableHeader className="sticky top-0 z-10">
                                <TableRow className="border-0 bg-emerald-700/95 hover:bg-emerald-700/95 [&>th]:border-r [&>th]:border-emerald-600 [&>th]:last:border-r-0">
                                    <SortableTableHead column="lot_number" label="LOT NUMBER" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[92px]" isText />
                                    <SortableTableHead column="sale_order" label="SALE ORDER" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[92px]" />
                                    <SortableTableHead column="title" label="TITLE" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[280px]" isText />
                                    <SortableTableHead column="retail_price" label="RETAIL PRICE" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[110px] text-right" />
                                    <SortableTableHead
                                        column="source"
                                        label="SOURCE"
                                        sortConfig={sortConfig}
                                        onSort={handleSort}
                                        className="!text-white p-0 min-w-[145px]"
                                        isText
                                        extraItems={[
                                            ...pricingVendors.map(v => ({
                                                label: v,
                                                onClick: () => handleSort('source', sortConfig?.direction || 'asc', v),
                                                isActive: sortConfig?.column === 'source' && sortConfig?.pinnedValue === v
                                            })),
                                            {
                                                label: 'Unspecified',
                                                onClick: () => handleSort('source', sortConfig?.direction || 'asc', 'Unknown'),
                                                isActive: sortConfig?.column === 'source' && sortConfig?.pinnedValue === 'Unknown'
                                            }
                                        ]}
                                    />
                                    <SortableTableHead column="condition" label="CONDITION" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[145px]" isText />
                                    <SortableTableHead column="cost_pct" label="COST" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[95px] text-right" />
                                    <SortableTableHead column="min_price_pct" label="MIN PRICE %" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[95px] text-right" />
                                    <SortableTableHead column="cost_price" label="COST PRICE" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[110px] text-right" />
                                    <SortableTableHead column="min_price_half" label="MIN PR (+0,5)" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[120px] text-right" />
                                    <SortableTableHead column="min_price_one" label="MIN PR (+1)" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[115px] text-right" />
                                    <SortableTableHead column="repeat" label="ПОВТОР" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[95px] text-right" />
                                    {historyHeaders.map((header) => (
                                        <SortableTableHead
                                            key={header}
                                            column={header}
                                            label={header}
                                            sortConfig={sortConfig}
                                            onSort={handleSort}
                                            className="!text-white p-0 min-w-[90px] text-right"
                                        />
                                    ))}
                                    {auction.status === 'Completed' && (
                                        <SortableTableHead column="buybacker" label="BUY-BACKER" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[135px]" isText />
                                    )}
                                    <SortableTableHead column="current_status" label="STATUS" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[115px]" isText />
                                    <TableHead className="h-9 min-w-[68px] p-0 !text-white"></TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {sortedRows.length === 0 ? (
                                    <TableRow className="border-0">
                                        <TableCell colSpan={tableColumnCount} className="h-24 text-center text-muted-foreground">
                                            No items found
                                        </TableCell>
                                    </TableRow>
                                ) : (
                                    sortedRows.map((rowData) => {
                                        const item = rowData.item;
                                        return (
                                            <TableRow key={item.id} className="group border-0 odd:bg-white even:bg-emerald-50/20 hover:bg-emerald-50/60 [&>td]:border-r [&>td]:border-b [&>td]:border-emerald-100 [&>td]:last:border-r-0">
                                                <TableCell className="px-2 py-1.5 font-medium font-mono">{rowData.lotNumber || '-'}</TableCell>
                                                <TableCell className="px-2 py-1.5 text-center font-mono">{rowData.saleOrder === '' ? '-' : rowData.saleOrder}</TableCell>
                                                <TableCell className="px-2 py-1.5 max-w-[280px]">
                                                    <div className="truncate text-sm font-medium">{rowData.title}</div>
                                                    <div className="text-[10px] text-muted-foreground truncate">{item.vendor_code || ''}</div>
                                                </TableCell>
                                                <TableCell className="px-2 py-1.5 text-right font-mono">{formatNumber(rowData.retailPrice)}</TableCell>
                                                <TableCell className="px-2 py-1.5">
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
                                                            placeholder="Set"
                                                        />
                                                    </div>
                                                </TableCell>
                                                <TableCell className="px-2 py-1.5">
                                                    <InlineSelect
                                                        value={item.condition || ''}
                                                        options={conditionOptions}
                                                        onChange={(val) => handleConditionChange(item.id, val)}
                                                        placeholder="Set"
                                                    />
                                                </TableCell>
                                                <TableCell className="px-2 py-1.5 text-right font-mono text-emerald-700">{Math.round(rowData.costPct * 100)}%</TableCell>
                                                <TableCell className="px-2 py-1.5 text-right font-mono text-amber-700">{Math.round(rowData.minPricePct * 100)}%</TableCell>
                                                <TableCell className="px-2 py-1.5 text-right font-mono">{formatNumber(Math.round(rowData.costPrice))}</TableCell>
                                                <TableCell className="px-2 py-1.5 text-right font-mono">{formatNumber(Math.round(rowData.minPriceHalf))}</TableCell>
                                                <TableCell className="px-2 py-1.5 text-right font-mono font-semibold text-emerald-700">{formatNumber(Math.round(rowData.minPriceOne))}</TableCell>
                                                <TableCell className="px-2 py-1.5 text-right font-mono font-semibold text-indigo-700">
                                                    {formatNumber(Math.round(rowData.repeat))}
                                                </TableCell>
                                                {historyHeaders.map((header) => (
                                                    <TableCell key={`${item.id}-${header}`} className="px-2 py-1.5 text-right font-mono">
                                                        {formatNumber(Math.round(rowData.historyBySeason[header] ?? 0))}
                                                    </TableCell>
                                                ))}
                                                {auction.status === 'Completed' && (
                                                    <TableCell className="px-2 py-1.5">
                                                        <InlineSelect
                                                            value={item.buybacker_id || ''}
                                                            options={buybackerOptions}
                                                            onChange={(val) => handleBuybackerChange(item.id, val)}
                                                            placeholder="--"
                                                        />
                                                    </TableCell>
                                                )}
                                                <TableCell className="px-2 py-1.5">
                                                    <Badge variant="secondary" className={getStatusColor(item.current_status)}>
                                                        {item.current_status}
                                                    </Badge>
                                                </TableCell>
                                                <TableCell className="px-1 py-1.5">
                                                    <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity justify-center">
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
                                        );
                                    })
                                )}
                            </TableBody>
                        </Table>
                    </div>
                    <div className="mt-4 text-xs text-muted-foreground text-center">
                        Showing {sortedRows.length} of {auctionItems.length} items
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}

