import { useEffect, useState, useMemo, useCallback } from 'react';
import { api } from '@/lib/api';
import type { InventoryItem, ConditionType, SourceType } from '@/types';
import { SortableTableHead, type SortConfig } from '@/components/ui/sortable-table-head';
import { Search, Download, MoreHorizontal, Package } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
} from '@/components/ui/table';
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { toast } from 'sonner';
import { formatCurrency, formatDate, naturalSort } from '@/lib/utils';
import { InventoryItemDetailDialog } from '@/components/inventory/InventoryItemDetailDialog';

export function InventoryPage() {
    const [items, setItems] = useState<InventoryItem[]>([]);
    const [loading, setLoading] = useState(true);
    const [searchTerm, setSearchTerm] = useState('');
    const [statusFilter, setStatusFilter] = useState<string>('all');

    // Reference data for inline editing
    const [conditionTypes, setConditionTypes] = useState<ConditionType[]>([]);
    const [sourceTypes, setSourceTypes] = useState<SourceType[]>([]);

    // Add state for details dialog
    const [selectedItem, setSelectedItem] = useState<InventoryItem | null>(null);
    const [detailOpen, setDetailOpen] = useState(false);

    // Sorting
    const [sortConfig, setSortConfig] = useState<SortConfig | null>(null);
    const handleSort = useCallback((column: string, direction: 'asc' | 'desc') => {
        setSortConfig({ column, direction });
    }, []);

    useEffect(() => {
        loadItems();
        loadReferenceData();
    }, []);

    const loadItems = async () => {
        try {
            setLoading(true);
            const data = await api.getInventoryItems();
            setItems(data);
        } catch (err) {
            console.error('Failed to load inventory:', err);
        } finally {
            setLoading(false);
        }
    };

    const loadReferenceData = async () => {
        try {
            const [ct, st] = await Promise.all([
                api.getConditionTypes(),
                api.getSourceTypes(),
            ]);
            setConditionTypes(ct);
            setSourceTypes(st);
        } catch (err) {
            console.error('Failed to load reference data:', err);
        }
    };

    const handleExportCsv = async () => {
        try {
            const savePath = await api.saveFile('inventory_export.csv');
            if (!savePath) return; // cancelled
            const count = await api.exportInventoryCsv(savePath as string, statusFilter !== 'all' ? statusFilter : undefined);
            toast.success(`Exported ${count} items to CSV`);
        } catch (err) {
            console.error('Failed to export CSV:', err);
            toast.error('Failed to export inventory');
        }
    };

    // Inline editing handlers
    const handleConditionChange = async (itemId: string, condition: string) => {
        try {
            await api.updateItemCondition(itemId, condition);
            setItems(prev => prev.map(i => i.id === itemId ? { ...i, condition } : i));
        } catch { toast.error('Failed to update condition'); }
    };

    const handleSourceChange = async (itemId: string, source: string) => {
        try {
            await api.updateItemSource(itemId, source);
            setItems(prev => prev.map(i => i.id === itemId ? { ...i, source } : i));
        } catch { toast.error('Failed to update source'); }
    };

    const filteredItems = useMemo(() => {
        return items.filter(item => {
            const matchesSearch = item.raw_title.toLowerCase().includes(searchTerm.toLowerCase()) ||
                item.lot_number?.toLowerCase().includes(searchTerm.toLowerCase());
            
            let matchesStatus = false;
            if (statusFilter === 'all') {
                matchesStatus = item.current_status !== 'Sold' && item.current_status !== 'Scrap';
            } else {
                matchesStatus = item.current_status === statusFilter;
            }

            return matchesSearch && matchesStatus;
        });
    }, [items, searchTerm, statusFilter]);

    const sortedItems = useMemo(() => {
        if (!sortConfig) return filteredItems;
        const { column, direction } = sortConfig;
        const sorted = [...filteredItems].sort((a, b) => {
            let aVal: any;
            let bVal: any;
            switch (column) {
                case 'lot_number': return direction === 'asc' ? naturalSort(a.lot_number || '', b.lot_number || '') : naturalSort(b.lot_number || '', a.lot_number || '');
                case 'raw_title': aVal = a.raw_title.toLowerCase(); bVal = b.raw_title.toLowerCase(); break;
                case 'condition': aVal = (a.condition || '').toLowerCase(); bVal = (b.condition || '').toLowerCase(); break;
                case 'source': aVal = (a.source || '').toLowerCase(); bVal = (b.source || '').toLowerCase(); break;
                case 'current_status': aVal = a.current_status.toLowerCase(); bVal = b.current_status.toLowerCase(); break;
                case 'retail_price': aVal = a.retail_price || 0; bVal = b.retail_price || 0; break;
                case 'cost_price': aVal = a.cost_price || 0; bVal = b.cost_price || 0; break;
                case 'min_price': aVal = a.min_price || 0; bVal = b.min_price || 0; break;
                case 'created_at': aVal = a.created_at || ''; bVal = b.created_at || ''; break;
                default: return 0;
            }
            if (aVal < bVal) return direction === 'asc' ? -1 : 1;
            if (aVal > bVal) return direction === 'asc' ? 1 : -1;
            return 0;
        });
        return sorted;
    }, [filteredItems, sortConfig]);

    const getStatusColor = (status: string) => {
        switch (status) {
            case 'InStock': return 'bg-blue-500/15 text-blue-700 dark:text-blue-300 hover:bg-blue-500/25';
            case 'Listed': return 'bg-amber-500/15 text-amber-700 dark:text-amber-300 hover:bg-amber-500/25';
            case 'Sold': return 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300 hover:bg-emerald-500/25';
            case 'Buyback': return 'bg-red-500/15 text-red-700 dark:text-red-300 hover:bg-red-500/25';
            case 'FloorSale': return 'bg-orange-500/15 text-orange-700 dark:text-orange-300 hover:bg-orange-500/25';
            case 'Unsold': return 'bg-gray-500/15 text-gray-700 dark:text-gray-300 hover:bg-gray-500/25';
            default: return 'bg-gray-500/15 text-gray-700 dark:text-gray-300';
        }
    };

    return (
        <div className="space-y-8 animate-fade-in">
            <InventoryItemDetailDialog
                item={selectedItem}
                open={detailOpen}
                onOpenChange={setDetailOpen}
            />

            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Inventory</h1>
                    <p className="text-muted-foreground mt-1">
                        Manage your stock ({items.length} total items)
                    </p>
                </div>
                <div className="flex gap-3">
                    <Button variant="outline" onClick={handleExportCsv}>
                        <Download className="mr-2 h-4 w-4" />
                        Export CSV
                    </Button>
                    <Button onClick={() => loadItems()}>Refresh</Button>
                </div>
            </div>

            <div className="flex items-center justify-between gap-4 bg-background/50 p-1 rounded-lg backdrop-blur-sm sticky top-0 z-10">
                <div className="relative flex-1 max-w-sm">
                    <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                    <Input
                        placeholder="Search by title, lot number..."
                        className="pl-9 bg-background/80"
                        value={searchTerm}
                        onChange={(e) => setSearchTerm(e.target.value)}
                    />
                </div>
                <div className="flex gap-2">
                    {['all', 'InStock', 'Listed', 'Buyback', 'FloorSale', 'Unsold', 'Scrap'].map((status) => (
                        <Button
                            key={status}
                            variant={statusFilter === status ? 'default' : 'outline'}
                            size="sm"
                            onClick={() => setStatusFilter(status)}
                            className="capitalize"
                        >
                            {status === 'all' ? 'Active Stock' : status === 'FloorSale' ? 'Floor Sales' : status}
                        </Button>
                    ))}
                </div>
            </div>

            <Card>
                <CardContent className="p-0">
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <SortableTableHead column="lot_number" label="Lot #" sortConfig={sortConfig} onSort={handleSort} className="w-[100px]" isText />
                                <SortableTableHead column="raw_title" label="Title" sortConfig={sortConfig} onSort={handleSort} isText />
                                <SortableTableHead column="condition" label="Condition" sortConfig={sortConfig} onSort={handleSort} className="w-[140px]" isText />
                                <SortableTableHead column="source" label="Source" sortConfig={sortConfig} onSort={handleSort} className="w-[120px]" isText />
                                <SortableTableHead column="current_status" label="Status" sortConfig={sortConfig} onSort={handleSort} isText />
                                <SortableTableHead column="retail_price" label="Retail" sortConfig={sortConfig} onSort={handleSort} className="text-right" />
                                <SortableTableHead column="cost_price" label="Cost" sortConfig={sortConfig} onSort={handleSort} className="text-right" />
                                <SortableTableHead column="min_price" label="Min Price" sortConfig={sortConfig} onSort={handleSort} className="text-right" />
                                <SortableTableHead column="created_at" label="Date" sortConfig={sortConfig} onSort={handleSort} className="text-right" isText />
                                <TableHead className="w-[50px]"></TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {loading ? (
                                <TableRow>
                                    <TableCell colSpan={10} className="h-24 text-center">
                                        Loading inventory...
                                    </TableCell>
                                </TableRow>
                            ) : filteredItems.length === 0 ? (
                                <TableRow>
                                    <TableCell colSpan={10} className="h-64 text-center">
                                        <div className="flex flex-col items-center justify-center text-muted-foreground">
                                            <Package className="h-12 w-12 mb-3 opacity-20" />
                                            <p>No items found matching your filters</p>
                                        </div>
                                    </TableCell>
                                </TableRow>
                            ) : (
                                sortedItems.map((item) => (
                                    <TableRow key={item.id} className="group">
                                        <TableCell className="font-medium font-mono">
                                            {item.lot_number || '-'}
                                        </TableCell>
                                        <TableCell className="max-w-[300px]">
                                            <div className="truncate font-medium">{item.raw_title}</div>
                                            <div className="text-xs text-muted-foreground truncate">
                                                {item.id.split('-')[0]}
                                            </div>
                                        </TableCell>
                                        {/* Inline Condition Dropdown */}
                                        <TableCell>
                                            <select
                                                className="w-full bg-transparent border border-transparent hover:border-border rounded px-1 py-0.5 text-xs cursor-pointer focus:outline-none focus:ring-1 focus:ring-ring transition-colors"
                                                value={item.condition || ''}
                                                onChange={(e) => handleConditionChange(item.id, e.target.value)}
                                            >
                                                <option value="">— Set —</option>
                                                {conditionTypes.map(ct => (
                                                    <option key={ct.id} value={ct.label}>{ct.label}</option>
                                                ))}
                                            </select>
                                        </TableCell>
                                        {/* Inline Source Dropdown */}
                                        <TableCell>
                                            <select
                                                className="w-full bg-transparent border border-transparent hover:border-border rounded px-1 py-0.5 text-xs cursor-pointer focus:outline-none focus:ring-1 focus:ring-ring transition-colors"
                                                value={item.source || ''}
                                                onChange={(e) => handleSourceChange(item.id, e.target.value)}
                                            >
                                                <option value="">— Set —</option>
                                                {sourceTypes.map(st => (
                                                    <option key={st.id} value={st.name}>{st.name}</option>
                                                ))}
                                            </select>
                                        </TableCell>
                                        <TableCell>
                                            <Badge variant="secondary" className={getStatusColor(item.current_status)}>
                                                {item.current_status}
                                            </Badge>
                                        </TableCell>
                                        <TableCell className="text-right font-medium">
                                            {formatCurrency(item.retail_price)}
                                        </TableCell>
                                        <TableCell className="text-right text-muted-foreground">
                                            {formatCurrency(item.cost_price)}
                                        </TableCell>
                                        <TableCell className="text-right text-emerald-600 font-medium">
                                            {formatCurrency(item.min_price)}
                                        </TableCell>
                                        <TableCell className="text-right text-muted-foreground text-xs">
                                            {formatDate(item.created_at || new Date().toISOString())}
                                        </TableCell>
                                        <TableCell>
                                            <DropdownMenu>
                                                <DropdownMenuTrigger asChild>
                                                    <Button variant="ghost" size="icon" className="opacity-0 group-hover:opacity-100 transition-opacity">
                                                        <MoreHorizontal className="h-4 w-4" />
                                                    </Button>
                                                </DropdownMenuTrigger>
                                                <DropdownMenuContent align="end">
                                                    <DropdownMenuLabel>Actions</DropdownMenuLabel>
                                                    <DropdownMenuItem
                                                        onClick={() => {
                                                            setSelectedItem(item);
                                                            setDetailOpen(true);
                                                        }}
                                                    >
                                                        View Details
                                                    </DropdownMenuItem>
                                                    {item.auction_id && item.current_status === 'Listed' && (
                                                        <DropdownMenuItem
                                                            onClick={async () => {
                                                                try {
                                                                    await api.updateItemStatus(item.id, 'InStock');
                                                                    toast.success('Removed from auction');
                                                                    loadItems();
                                                                } catch (err) {
                                                                    toast.error('Failed to remove from auction');
                                                                }
                                                            }}
                                                        >
                                                            Remove from Auction
                                                        </DropdownMenuItem>
                                                    )}
                                                    <DropdownMenuSeparator />
                                                    <DropdownMenuItem
                                                        className="text-red-600 focus:bg-red-50 focus:text-red-700"
                                                        onClick={async () => {
                                                            try {
                                                                await api.updateItemStatus(item.id, 'Scrap');
                                                                toast.success('Item marked as scrap');
                                                                loadItems();
                                                            } catch (err) {
                                                                toast.error('Failed to mark as scrap');
                                                            }
                                                        }}
                                                    >
                                                        Mark as Scrap
                                                    </DropdownMenuItem>
                                                </DropdownMenuContent>
                                            </DropdownMenu>
                                        </TableCell>
                                    </TableRow>
                                ))
                            )}
                        </TableBody>
                    </Table>
                </CardContent>
            </Card>

            <div className="text-xs text-muted-foreground text-center">
                Showing {sortedItems.length} of {items.length} items
            </div>
        </div>
    );
}
