import { useEffect, useState, useMemo, useCallback } from 'react';
import { api } from '@/lib/api';
import type { InventoryItem, ConditionType, SourceType, Auction } from '@/types';
import { SortableTableHead, type SortConfig } from '@/components/ui/sortable-table-head';
import { Search, MoreHorizontal, Package } from 'lucide-react';
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
import { formatNumber, naturalSort } from '@/lib/utils';
import { InventoryItemDetailDialog } from '@/components/inventory/InventoryItemDetailDialog';
import { getHistoryHeadersForAuction } from '@/lib/managerReport';
import { extractAuctionNumber } from '@/lib/auctionNaming';

function InlineSelect({
    value,
    options,
    onChange,
    placeholder,
}: {
    value: string;
    options: { value: string; label: string }[];
    onChange: (val: string) => void;
    placeholder?: string;
}) {
    return (
        <select
            className="w-full bg-transparent border border-transparent hover:border-border rounded px-1 py-0.5 text-xs cursor-pointer focus:outline-none focus:ring-1 focus:ring-ring transition-colors"
            value={value}
            onChange={(e) => onChange(e.target.value)}
        >
            {placeholder && <option value="">{placeholder}</option>}
            {options.map((opt) => (
                <option key={opt.value} value={opt.value}>
                    {opt.label}
                </option>
            ))}
        </select>
    );
}

export function InventoryPage() {
    const [items, setItems] = useState<InventoryItem[]>([]);
    const [auctions, setAuctions] = useState<Auction[]>([]);
    const [loading, setLoading] = useState(true);
    const [searchTerm, setSearchTerm] = useState('');
    const [conditionTypes, setConditionTypes] = useState<ConditionType[]>([]);
    const [sourceTypes, setSourceTypes] = useState<SourceType[]>([]);
    const [repeaterStatsByTitle, setRepeaterStatsByTitle] = useState<Record<string, Record<string, number>>>({});
    const [firstAuctionByItemId, setFirstAuctionByItemId] = useState<Record<string, string>>({});
    const [selectedItem, setSelectedItem] = useState<InventoryItem | null>(null);
    const [detailOpen, setDetailOpen] = useState(false);
    const [sortConfig, setSortConfig] = useState<SortConfig | null>(null);

    const handleSort = useCallback((column: string, direction: 'asc' | 'desc') => {
        setSortConfig({ column, direction });
    }, []);

    useEffect(() => {
        loadItems();
        loadReferenceData();
        loadAuctions();
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

    const loadAuctions = async () => {
        try {
            const data = await api.getAuctions();
            setAuctions(data);
        } catch (err) {
            console.error('Failed to load auctions:', err);
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

    const historyHeaders = useMemo(() => {
        if (auctions.length === 0) return getHistoryHeadersForAuction();

        const auctionsWithNumber = auctions
            .map((auction) => {
                const numberText = extractAuctionNumber(auction.name);
                const number = numberText ? Number.parseInt(numberText, 10) : null;
                return { auction, number: Number.isFinite(number) ? number : null };
            })
            .filter((entry): entry is { auction: Auction; number: number } => entry.number !== null);

        if (auctionsWithNumber.length > 0) {
            const latestByNumber = [...auctionsWithNumber].sort((a, b) => b.number - a.number)[0];
            return getHistoryHeadersForAuction(latestByNumber.auction.name);
        }

        const latestByDate = [...auctions].sort(
            (a, b) => (b.created_at || '').localeCompare(a.created_at || ''),
        )[0];
        return getHistoryHeadersForAuction(latestByDate?.name);
    }, [auctions]);

    const auctionNameById = useMemo(() => {
        return auctions.reduce<Record<string, string>>((acc, auction) => {
            acc[auction.id] = auction.name;
            return acc;
        }, {});
    }, [auctions]);

    useEffect(() => {
        let cancelled = false;

        const loadSupplementalData = async () => {
            if (items.length === 0) {
                if (!cancelled) {
                    setRepeaterStatsByTitle({});
                    setFirstAuctionByItemId({});
                }
                return;
            }

            const normalizedTitles = Array.from(
                new Set(
                    items
                        .map((item) => item.normalized_title?.trim())
                        .filter((title): title is string => Boolean(title)),
                ),
            );
            const itemIds = items.map((item) => item.id);

            try {
                const [stats, firstAuctionMap] = await Promise.all([
                    normalizedTitles.length > 0
                        ? api.getItemRepeaterStats(normalizedTitles, historyHeaders)
                        : Promise.resolve({}),
                    itemIds.length > 0
                        ? api.getItemFirstAuctionMap(itemIds)
                        : Promise.resolve({}),
                ]);

                if (!cancelled) {
                    setRepeaterStatsByTitle(stats || {});
                    setFirstAuctionByItemId(firstAuctionMap || {});
                }
            } catch (err) {
                console.error('Failed to load inventory repeaters:', err);
                if (!cancelled) {
                    setRepeaterStatsByTitle({});
                    setFirstAuctionByItemId({});
                }
            }
        };

        loadSupplementalData();
        return () => {
            cancelled = true;
        };
    }, [items, historyHeaders]);

    const handleConditionChange = async (itemId: string, condition: string) => {
        try {
            await api.updateItemCondition(itemId, condition);
            setItems((prev) => prev.map((item) => (item.id === itemId ? { ...item, condition } : item)));
        } catch {
            toast.error('Failed to update condition');
        }
    };

    const handleSourceChange = async (itemId: string, source: string) => {
        try {
            await api.updateItemSource(itemId, source);
            setItems((prev) => prev.map((item) => (item.id === itemId ? { ...item, source } : item)));
        } catch {
            toast.error('Failed to update source');
        }
    };

    const getHistoryValue = useCallback((item: InventoryItem, header: string) => {
        if (!item.normalized_title) return 0;
        return repeaterStatsByTitle[item.normalized_title]?.[header] ?? 0;
    }, [repeaterStatsByTitle]);

    const getRepeatValue = useCallback((item: InventoryItem) => {
        if (historyHeaders.length === 0) return 0;
        return Math.max(...historyHeaders.map((header) => getHistoryValue(item, header)));
    }, [getHistoryValue, historyHeaders]);

    const getFirstAuctionName = useCallback((item: InventoryItem) => {
        const direct = firstAuctionByItemId[item.id];
        if (direct) return direct;
        if (item.auction_id) return auctionNameById[item.auction_id] || '';
        return '';
    }, [firstAuctionByItemId, auctionNameById]);

    const filteredItems = useMemo(() => {
        return items.filter((item) => {
            const matchesSearch = item.raw_title.toLowerCase().includes(searchTerm.toLowerCase()) ||
                item.lot_number?.toLowerCase().includes(searchTerm.toLowerCase());

            const matchesStatus = item.current_status !== 'Sold' && item.current_status !== 'Scrap';

            return matchesSearch && matchesStatus;
        });
    }, [items, searchTerm]);

    const sortedItems = useMemo(() => {
        if (!sortConfig) return filteredItems;

        const { column, direction } = sortConfig;
        return [...filteredItems].sort((a, b) => {
            let aVal: string | number = '';
            let bVal: string | number = '';

            if (historyHeaders.includes(column)) {
                aVal = getHistoryValue(a, column);
                bVal = getHistoryValue(b, column);
            } else {
                switch (column) {
                    case 'lot_number':
                        return direction === 'asc'
                            ? naturalSort(a.lot_number || '', b.lot_number || '')
                            : naturalSort(b.lot_number || '', a.lot_number || '');
                    case 'raw_title':
                        aVal = a.raw_title.toLowerCase();
                        bVal = b.raw_title.toLowerCase();
                        break;
                    case 'condition':
                        aVal = (a.condition || '').toLowerCase();
                        bVal = (b.condition || '').toLowerCase();
                        break;
                    case 'source':
                        aVal = (a.source || '').toLowerCase();
                        bVal = (b.source || '').toLowerCase();
                        break;
                    case 'current_status':
                        aVal = a.current_status.toLowerCase();
                        bVal = b.current_status.toLowerCase();
                        break;
                    case 'first_auction':
                        aVal = getFirstAuctionName(a).toLowerCase();
                        bVal = getFirstAuctionName(b).toLowerCase();
                        break;
                    case 'min_price':
                        aVal = a.min_price || 0;
                        bVal = b.min_price || 0;
                        break;
                    case 'repeat':
                        aVal = getRepeatValue(a);
                        bVal = getRepeatValue(b);
                        break;
                    default:
                        return 0;
                }
            }

            if (aVal < bVal) return direction === 'asc' ? -1 : 1;
            if (aVal > bVal) return direction === 'asc' ? 1 : -1;
            return 0;
        });
    }, [filteredItems, sortConfig, getFirstAuctionName, getRepeatValue, getHistoryValue, historyHeaders]);

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

    const conditionOptions = useMemo(
        () => conditionTypes.map((type) => ({ value: type.label, label: type.label })),
        [conditionTypes],
    );
    const sourceOptions = useMemo(
        () => [...sourceTypes]
            .sort((a, b) => a.name.localeCompare(b.name))
            .map((source) => ({ value: source.name, label: source.name })),
        [sourceTypes],
    );

    const tableColumnCount = 9 + historyHeaders.length;

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
                    <Button
                        onClick={() => {
                            loadItems();
                            loadAuctions();
                        }}
                    >
                        Refresh
                    </Button>
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
            </div>

            <Card className="border-emerald-200/70 shadow-sm">
                <CardContent className="p-0">
                    <div className="overflow-x-auto rounded-xl border border-emerald-200/80 bg-gradient-to-b from-emerald-50/30 to-background shadow-inner">
                        <Table className="min-w-[1700px] text-xs border-separate border-spacing-0">
                            <TableHeader className="sticky top-0 z-10">
                                <TableRow className="border-0 bg-emerald-700/95 hover:bg-emerald-700/95 [&>th]:border-r [&>th]:border-emerald-600 [&>th]:last:border-r-0">
                                    <SortableTableHead column="lot_number" label="LOT #" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[92px]" isText />
                                    <SortableTableHead column="raw_title" label="TITLE" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[280px]" isText />
                                    <SortableTableHead column="condition" label="CONDITION" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[145px]" isText />
                                    <SortableTableHead column="source" label="SOURCE" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[145px]" isText />
                                    <SortableTableHead column="current_status" label="STATUS" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[115px]" isText />
                                    <SortableTableHead column="first_auction" label="FIRST AUCTION" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[150px]" isText />
                                    <SortableTableHead column="min_price" label="MIN PRICE" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[115px] text-right" />
                                    <SortableTableHead column="repeat" label="REPEAT" sortConfig={sortConfig} onSort={handleSort} className="!text-white p-0 min-w-[95px] text-right" />
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
                                    <TableHead className="h-9 min-w-[68px] p-0 !text-white"></TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {loading ? (
                                    <TableRow className="border-0">
                                        <TableCell colSpan={tableColumnCount} className="h-24 text-center text-muted-foreground">
                                            Loading inventory...
                                        </TableCell>
                                    </TableRow>
                                ) : sortedItems.length === 0 ? (
                                    <TableRow className="border-0">
                                        <TableCell colSpan={tableColumnCount} className="h-64 text-center">
                                            <div className="flex flex-col items-center justify-center text-muted-foreground">
                                                <Package className="h-12 w-12 mb-3 opacity-20" />
                                                <p>No items found matching your filters</p>
                                            </div>
                                        </TableCell>
                                    </TableRow>
                                ) : (
                                    sortedItems.map((item) => {
                                        const firstAuctionName = getFirstAuctionName(item);
                                        const repeatValue = getRepeatValue(item);
                                        return (
                                            <TableRow key={item.id} className="group border-0 odd:bg-white even:bg-emerald-50/20 hover:bg-emerald-50/60 [&>td]:border-r [&>td]:border-b [&>td]:border-emerald-100 [&>td]:last:border-r-0">
                                                <TableCell className="px-2 py-1.5 font-medium font-mono">{item.lot_number || '-'}</TableCell>
                                                <TableCell className="px-2 py-1.5 max-w-[280px]">
                                                    <div className="truncate text-sm font-medium">{item.raw_title}</div>
                                                    <div className="text-[10px] text-muted-foreground truncate">{item.vendor_code || item.id.split('-')[0]}</div>
                                                </TableCell>
                                                <TableCell className="px-2 py-1.5">
                                                    <InlineSelect
                                                        value={item.condition || ''}
                                                        options={conditionOptions}
                                                        onChange={(value) => handleConditionChange(item.id, value)}
                                                        placeholder="Set"
                                                    />
                                                </TableCell>
                                                <TableCell className="px-2 py-1.5">
                                                    <InlineSelect
                                                        value={item.source || ''}
                                                        options={sourceOptions}
                                                        onChange={(value) => handleSourceChange(item.id, value)}
                                                        placeholder="Set"
                                                    />
                                                </TableCell>
                                                <TableCell className="px-2 py-1.5">
                                                    <Badge variant="secondary" className={getStatusColor(item.current_status)}>
                                                        {item.current_status}
                                                    </Badge>
                                                </TableCell>
                                                <TableCell className="px-2 py-1.5 font-medium text-xs">
                                                    {firstAuctionName || '-'}
                                                </TableCell>
                                                <TableCell className="px-2 py-1.5 text-right font-mono font-semibold text-emerald-700">
                                                    {formatNumber(Math.round(item.min_price || 0))}
                                                </TableCell>
                                                <TableCell className="px-2 py-1.5 text-right font-mono font-semibold text-indigo-700">
                                                    {formatNumber(Math.round(repeatValue))}
                                                </TableCell>
                                                {historyHeaders.map((header) => (
                                                    <TableCell key={`${item.id}-${header}`} className="px-2 py-1.5 text-right font-mono">
                                                        {formatNumber(Math.round(getHistoryValue(item, header)))}
                                                    </TableCell>
                                                ))}
                                                <TableCell className="px-1 py-1.5">
                                                    <DropdownMenu>
                                                        <DropdownMenuTrigger asChild>
                                                            <Button variant="ghost" size="icon" className="h-7 w-7 opacity-0 group-hover:opacity-100 transition-opacity">
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
                                                                        } catch {
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
                                                                    } catch {
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
                                        );
                                    })
                                )}
                            </TableBody>
                        </Table>
                    </div>
                </CardContent>
            </Card>

            <div className="text-xs text-muted-foreground text-center">
                Showing {sortedItems.length} of {items.length} items
            </div>
        </div>
    );
}
