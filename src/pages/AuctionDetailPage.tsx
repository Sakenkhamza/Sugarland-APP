import { useEffect, useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { api } from '@/lib/api';
import type { Auction, InventoryItem } from '@/types';
import { ArrowLeft, Download, Plus, X, Search, CheckCircle2 } from 'lucide-react';
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
import { formatCurrency } from '@/lib/utils';
import { toast } from 'sonner';

export function AuctionDetailPage() {
    const { id } = useParams<{ id: string }>();
    const navigate = useNavigate();

    const [auction, setAuction] = useState<Auction | null>(null);
    const [assignedItems, setAssignedItems] = useState<InventoryItem[]>([]);
    const [availableItems, setAvailableItems] = useState<InventoryItem[]>([]);

    const [loading, setLoading] = useState(true);
    const [searchQuery, setSearchQuery] = useState('');
    const [selectedItemsToAdd, setSelectedItemsToAdd] = useState<Set<string>>(new Set());
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

            setAssignedItems(allItems.filter(item => item.auction_id === auctionId));
            setAvailableItems(allItems.filter(item => item.current_status === 'InStock' && !item.auction_id));

            setSelectedItemsToAdd(new Set());
        } catch (err) {
            console.error('Failed to load auction data:', err);
            toast.error('Failed to load auction data');
        } finally {
            setLoading(false);
        }
    };

    const handleUpdateStatus = async (newStatus: string) => {
        if (!auction) return;
        setIsUpdatingStatus(true);
        try {
            await api.updateAuctionStatus(auction.id, newStatus);
            setAuction({ ...auction, status: newStatus as any });
            toast.success(`Auction marked as ${newStatus}`);
        } catch (error) {
            console.error('Failed to update status:', error);
            toast.error('Failed to update status');
        } finally {
            setIsUpdatingStatus(false);
        }
    };

    const handleExportCsv = async () => {
        if (!auction) return;
        try {
            const defaultName = `${auction.name.replace(/\s+/g, '_')}_lots.csv`;
            const savePath = await api.saveFile(defaultName);
            if (!savePath) return; // User cancelled

            await api.exportAuctionCsv(auction.id, savePath as string);
            toast.success(`Exported lots to ${savePath}`);
        } catch (error) {
            console.error('Failed to export CSV:', error);
            toast.error('Failed to export CSV');
        }
    };

    const handleToggleAddItem = (itemId: string) => {
        const next = new Set(selectedItemsToAdd);
        if (next.has(itemId)) {
            next.delete(itemId);
        } else {
            next.add(itemId);
        }
        setSelectedItemsToAdd(next);
    };

    const handleAddSelected = async () => {
        if (!auction || selectedItemsToAdd.size === 0) return;
        try {
            await api.assignItemsToAuction(auction.id, Array.from(selectedItemsToAdd));
            toast.success(`${selectedItemsToAdd.size} items added to auction`);
            await loadAuctionData(auction.id);
        } catch (error) {
            console.error('Failed to assign items:', error);
            toast.error('Failed to add items to auction');
        }
    };

    if (loading) {
        return <div className="p-8 text-center text-muted-foreground">Loading auction details...</div>;
    }

    if (!auction) {
        return <div className="p-8 text-center text-red-500">Auction not found</div>;
    }

    const totalRetail = assignedItems.reduce((acc, item) => acc + (item.retail_price || 0), 0);
    const estRevenue = assignedItems.reduce((acc, item) => acc + (item.min_price || 0), 0);

    const filteredAvailableItems = availableItems.filter(item =>
        item.raw_title.toLowerCase().includes(searchQuery.toLowerCase()) ||
        item.lot_number?.toLowerCase().includes(searchQuery.toLowerCase())
    );

    const getStatusColor = (status: string) => {
        switch (status) {
            case 'Draft': return 'bg-gray-500/15 text-gray-700 dark:text-gray-300';
            case 'Active': return 'bg-blue-500/15 text-blue-700 dark:text-blue-300';
            case 'Completed': return 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300';
            case 'Cancelled': return 'bg-red-500/15 text-red-700 dark:text-red-300';
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
                    {auction.status === 'Draft' && (
                        <Button
                            variant="default"
                            onClick={() => handleUpdateStatus('Active')}
                            disabled={isUpdatingStatus}
                        >
                            Activate
                        </Button>
                    )}
                    {auction.status === 'Active' && (
                        <Button
                            variant="default"
                            onClick={() => handleUpdateStatus('Completed')}
                            disabled={isUpdatingStatus}
                            className="bg-emerald-600 hover:bg-emerald-700"
                        >
                            <CheckCircle2 className="mr-2 h-4 w-4" />
                            Mark Completed
                        </Button>
                    )}
                    {['Draft', 'Active', 'Completed'].includes(auction.status) && (
                        <Button
                            variant="outline"
                            onClick={handleExportCsv}
                            disabled={assignedItems.length === 0}
                        >
                            <Download className="mr-2 h-4 w-4" />
                            Export CSV
                        </Button>
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
                        <div className="text-2xl font-bold">{assignedItems.length}</div>
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium text-muted-foreground">Total Retail</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold text-emerald-600">{formatCurrency(totalRetail)}</div>
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium text-muted-foreground">Est. Revenue</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold text-blue-600">{formatCurrency(estRevenue)}</div>
                    </CardContent>
                </Card>
            </div>

            {/* Assigned Items Section */}
            <Card>
                <CardHeader>
                    <CardTitle>Assigned Items</CardTitle>
                </CardHeader>
                <CardContent>
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>Lot #</TableHead>
                                <TableHead>Title</TableHead>
                                <TableHead>Retail</TableHead>
                                <TableHead>Cost</TableHead>
                                <TableHead>Min Price</TableHead>
                                <TableHead>Status</TableHead>
                                {auction.status !== 'Completed' && <TableHead className="w-12"></TableHead>}
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {assignedItems.length === 0 ? (
                                <TableRow>
                                    <TableCell colSpan={7} className="h-24 text-center text-muted-foreground">
                                        No items assigned yet
                                    </TableCell>
                                </TableRow>
                            ) : (
                                assignedItems.map((item) => (
                                    <TableRow key={item.id}>
                                        <TableCell className="font-medium">{item.lot_number || '-'}</TableCell>
                                        <TableCell className="max-w-xs truncate">{item.raw_title}</TableCell>
                                        <TableCell>{formatCurrency(item.retail_price || 0)}</TableCell>
                                        <TableCell>{formatCurrency(item.cost_price || 0)}</TableCell>
                                        <TableCell>{formatCurrency(item.min_price || 0)}</TableCell>
                                        <TableCell>
                                            <Badge variant="outline">{item.current_status}</Badge>
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
                </CardContent>
            </Card>

            {/* Add Items Section */}
            {['Draft', 'Active'].includes(auction.status) && (
                <Card>
                    <CardHeader>
                        <div className="flex items-center justify-between">
                            <CardTitle>Available Inventory</CardTitle>
                            {selectedItemsToAdd.size > 0 && (
                                <Button size="sm" onClick={handleAddSelected}>
                                    <Plus className="mr-2 h-4 w-4" />
                                    Add Selected ({selectedItemsToAdd.size})
                                </Button>
                            )}
                        </div>
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <div className="relative">
                            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                            <Input
                                placeholder="Search available inventory by title or lot number..."
                                className="pl-9"
                                value={searchQuery}
                                onChange={(e) => setSearchQuery(e.target.value)}
                            />
                        </div>
                        <div className="border rounded-md">
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHead className="w-12"></TableHead>
                                        <TableHead>Lot #</TableHead>
                                        <TableHead>Title</TableHead>
                                        <TableHead>Source</TableHead>
                                        <TableHead>Retail</TableHead>
                                        <TableHead>Min Price</TableHead>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {filteredAvailableItems.length === 0 ? (
                                        <TableRow>
                                            <TableCell colSpan={6} className="h-24 text-center text-muted-foreground">
                                                No available inventory matches search
                                            </TableCell>
                                        </TableRow>
                                    ) : (
                                        filteredAvailableItems.map((item) => {
                                            const isSelected = selectedItemsToAdd.has(item.id);
                                            return (
                                                <TableRow key={item.id} className={isSelected ? 'bg-muted/50' : ''}>
                                                    <TableCell>
                                                        <div
                                                            className={`w-4 h-4 rounded border flex items-center justify-center cursor-pointer ${isSelected ? 'bg-primary border-primary' : 'border-input'}`}
                                                            onClick={() => handleToggleAddItem(item.id)}
                                                        >
                                                            {isSelected && <CheckCircle2 className="h-3 w-3 text-primary-foreground" />}
                                                        </div>
                                                    </TableCell>
                                                    <TableCell className="font-medium">{item.lot_number || '-'}</TableCell>
                                                    <TableCell className="max-w-xs truncate">{item.raw_title}</TableCell>
                                                    <TableCell>
                                                        <Badge variant="outline">{item.source}</Badge>
                                                    </TableCell>
                                                    <TableCell>{formatCurrency(item.retail_price || 0)}</TableCell>
                                                    <TableCell>{formatCurrency(item.min_price || 0)}</TableCell>
                                                </TableRow>
                                            );
                                        })
                                    )}
                                </TableBody>
                            </Table>
                        </div>
                    </CardContent>
                </Card>
            )}
        </div>
    );
}
