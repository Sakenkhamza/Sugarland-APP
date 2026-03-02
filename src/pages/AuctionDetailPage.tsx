import { useEffect, useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { api } from '@/lib/api';
import type { Auction, InventoryItem } from '@/types';
import { ArrowLeft, X, Search, Flag, FileSpreadsheet } from 'lucide-react';
import * as XLSX from 'xlsx';
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
import { formatCurrencyWhole } from '@/lib/utils';
import { toast } from 'sonner';

export function AuctionDetailPage() {
    const { id } = useParams<{ id: string }>();
    const navigate = useNavigate();

    const [auction, setAuction] = useState<Auction | null>(null);
    const [auctionItems, setAuctionItems] = useState<InventoryItem[]>([]);

    const [loading, setLoading] = useState(true);

    // Filters
    const [searchQuery, setSearchQuery] = useState('');
    const [statusFilter, setStatusFilter] = useState('all');
    const [vendorFilter, setVendorFilter] = useState('all');
    const [sortOrder, setSortOrder] = useState<'default' | 'retail_asc' | 'retail_desc'>('default');

    const [isUpdatingStatus, setIsUpdatingStatus] = useState(false);

    useEffect(() => {
        if (id) {
            loadAuctionData(id);
        }
    }, [id]);

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

    const handleFinish = async () => {
        if (!auction) return;

        // 1. Prompt user to select the HiBid results CSV file
        const filePath = await api.selectFile([
            { name: 'CSV Files', extensions: ['csv'] }
        ]);

        if (!filePath) {
            toast.info('No file selected — auction not finished');
            return;
        }

        // 2. Call finish with the selected CSV
        setIsUpdatingStatus(true);
        try {
            const result = await api.finishAuction(auction.id, filePath as string);
            setAuction({ ...auction, status: 'Completed' as any });
            toast.success(`Auction finished! Reports generated: ${result.detail_report}, ${result.summary_report}`);
        } catch (error) {
            console.error('Failed to finish auction:', error);
            toast.error('Failed to finish auction: ' + String(error));
        } finally {
            setIsUpdatingStatus(false);
        }
    };



    const handleExportExcel = async () => {
        if (!auction || auctionItems.length === 0) return;
        try {
            const formattedData = auctionItems.map((item) => {
                const retail = Math.round(item.retail_price || 0);
                const cost = Math.round(item.cost_price || 0);
                const min_price = Math.ceil(item.min_price || 0);

                const cost_pct = (item.retail_price || 0) > 0 ? Math.round(((item.cost_price || 0) / (item.retail_price || 1)) * 100) : 0;
                const min_pr_10_pct = Math.round((item.retail_price || 0) * 0.10);

                return {
                    'Auction name': auction.name,
                    'LotNumber': item.lot_number || '',
                    'Quantity': 1,
                    'Title': item.raw_title || '',
                    'Vendor Code': item.source === 'Best Buy' ? 'ATXSUGAR' : '',
                    'Retail Price': retail,
                    'Source': item.source || '',
                    'cost': cost_pct + '%',
                    'cost price': cost,
                    'Retail price': retail,
                    '% min pr (+10%)': min_pr_10_pct,
                    'min price': min_price
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
            console.error('Failed to export Excel:', error);
            toast.error('Failed to export Excel');
        }
    };

    if (loading) {
        return <div className="p-8 text-center text-muted-foreground">Loading auction details...</div>;
    }

    if (!auction) {
        return <div className="p-8 text-center text-red-500">Auction not found</div>;
    }

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
        if (sortOrder === 'retail_asc') {
            return (a.retail_price || 0) - (b.retail_price || 0);
        } else if (sortOrder === 'retail_desc') {
            return (b.retail_price || 0) - (a.retail_price || 0);
        }
        return 0;
    });

    const uniqueVendors = Array.from(new Set(auctionItems.map(i => i.source).filter(Boolean))) as string[];

    const getStatusColor = (status: string) => {
        switch (status) {
            case 'Active': return 'bg-blue-500/15 text-blue-700 dark:text-blue-300';
            case 'Completed': return 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300';
            default: return 'bg-gray-500/15 text-gray-700 dark:text-gray-300';
        }
    };

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
                    {auction.status === 'Active' && (
                        <Button
                            variant="default"
                            onClick={handleFinish}
                            disabled={isUpdatingStatus}
                            className="bg-emerald-600 hover:bg-emerald-700"
                        >
                            <Flag className="mr-2 h-4 w-4" />
                            {isUpdatingStatus ? 'Finishing...' : 'Finish'}
                        </Button>
                    )}
                    {['Active', 'Completed'].includes(auction.status) && (
                        <>
                            <Button
                                variant="outline"
                                onClick={handleExportExcel}
                                disabled={auctionItems.length === 0}
                                className="text-emerald-600 border-emerald-500 hover:bg-emerald-50 hover:text-emerald-700 w-9 px-0"
                            >
                                <FileSpreadsheet className="h-4 w-4" />
                            </Button>
                        </>
                    )}
                </div>
            </div>

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
                                className="flex h-10 items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                                value={sortOrder}
                                onChange={(e) => setSortOrder(e.target.value as any)}
                            >
                                <option value="default">Sort: Default</option>
                                <option value="retail_asc">Retail: Low to High</option>
                                <option value="retail_desc">Retail: High to Low</option>
                            </select>
                            <select
                                className="flex h-10 items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                                value={statusFilter}
                                onChange={(e) => setStatusFilter(e.target.value)}
                            >
                                <option value="all">All Statuses</option>
                                <option value="Listed">Listed</option>
                                <option value="Sold">Sold</option>
                                <option value="Buyback">Buyback</option>
                                <option value="Scrap">Scrap</option>
                            </select>
                            {uniqueVendors.length > 0 && (
                                <select
                                    className="flex h-10 items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                                    value={vendorFilter}
                                    onChange={(e) => setVendorFilter(e.target.value)}
                                >
                                    <option value="all">All Vendors</option>
                                    {uniqueVendors.map(v => (
                                        <option key={v} value={v}>{v}</option>
                                    ))}
                                </select>
                            )}
                        </div>
                    </div>
                </CardHeader>
                <CardContent>
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>Lot #</TableHead>
                                <TableHead>Title</TableHead>
                                <TableHead>Source</TableHead>
                                <TableHead>Vendor Code</TableHead>
                                <TableHead className="text-right">Retail</TableHead>
                                <TableHead className="text-right">Cost %</TableHead>
                                <TableHead className="text-right">Cost Price</TableHead>
                                <TableHead className="text-right">Min Pr (+10%)</TableHead>
                                <TableHead className="text-right">Min Price</TableHead>
                                <TableHead>Status</TableHead>
                                {auction.status !== 'Completed' && <TableHead className="w-12"></TableHead>}
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {sortedItems.length === 0 ? (
                                <TableRow>
                                    <TableCell colSpan={8} className="h-24 text-center text-muted-foreground">
                                        No items found
                                    </TableCell>
                                </TableRow>
                            ) : (
                                sortedItems.map((item) => (
                                    <TableRow key={item.id}>
                                        <TableCell className="font-medium">{item.lot_number || '-'}</TableCell>
                                        <TableCell className="max-w-xs truncate">{item.raw_title}</TableCell>
                                        <TableCell>
                                            <Badge variant="outline">{item.source || 'Unknown'}</Badge>
                                        </TableCell>
                                        <TableCell className="text-muted-foreground text-xs font-mono">
                                            {item.source === 'Best Buy' ? 'ATXSUGAR' : '-'}
                                        </TableCell>
                                        <TableCell className="text-right font-medium">{formatCurrencyWhole(item.retail_price || 0)}</TableCell>
                                        <TableCell className="text-right text-muted-foreground text-xs">
                                            {item.retail_price ? Math.round(((item.cost_price || 0) / item.retail_price) * 100) + '%' : '0%'}
                                        </TableCell>
                                        <TableCell className="text-right">{formatCurrencyWhole(item.cost_price || 0)}</TableCell>
                                        <TableCell className="text-right text-muted-foreground text-xs">
                                            {formatCurrencyWhole((item.retail_price || 0) * 0.10)}
                                        </TableCell>
                                        <TableCell className="text-right font-semibold text-emerald-600 dark:text-emerald-400">
                                            {formatCurrencyWhole(Math.ceil(item.min_price || 0))}
                                        </TableCell>
                                        <TableCell>
                                            <Badge variant="secondary" className={getStatusColor(item.current_status)}>
                                                {item.current_status}
                                            </Badge>
                                        </TableCell>
                                        {auction.status !== 'Completed' && (
                                            <TableCell>
                                                <Button variant="ghost" size="icon" className="h-8 w-8 text-muted-foreground hover:text-red-500" onClick={async () => {
                                                    try {
                                                        await api.unassignItem(item.id);
                                                        toast.success('Item removed from auction');
                                                        if (auction) {
                                                            await loadAuctionData(auction.id);
                                                        }
                                                    } catch (err) {
                                                        toast.error('Failed to remove item');
                                                    }
                                                }}>
                                                    <X className="h-4 w-4" />
                                                </Button>
                                            </TableCell>
                                        )}
                                    </TableRow>
                                ))
                            )}
                        </TableBody>
                    </Table>
                    <div className="mt-4 text-xs text-muted-foreground text-center">
                        Showing {sortedItems.length} of {auctionItems.length} items
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
