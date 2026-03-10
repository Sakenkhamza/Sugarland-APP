import { useEffect, useState } from 'react';
import { api } from '@/lib/api';
import type { Auction } from '@/types';
import * as XLSX from 'xlsx';
import { Plus, FileSpreadsheet, Calendar, Pencil, Trash2 } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
import { useNavigate } from 'react-router-dom';
import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
} from '@/components/ui/table';
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
    DialogFooter,
    DialogDescription,
} from '@/components/ui/dialog';
import { formatDate } from '@/lib/utils';
import { CreateAuctionDialog } from '@/components/auctions/CreateAuctionDialog';
import { toast } from 'sonner';

export function AuctionsPage() {
    const navigate = useNavigate();
    const [auctions, setAuctions] = useState<Auction[]>([]);
    const [loading, setLoading] = useState(true);
    const [isDialogOpen, setIsDialogOpen] = useState(false);

    // Edit dialog state
    const [editTarget, setEditTarget] = useState<Auction | null>(null);
    const [editName, setEditName] = useState('');
    const [editSaving, setEditSaving] = useState(false);
    const [deleteConfirm, setDeleteConfirm] = useState(false);

    useEffect(() => {
        loadAuctions();
    }, []);

    const loadAuctions = async () => {
        try {
            setLoading(true);
            const data = await api.getAuctions();
            setAuctions(data);
        } catch (err) {
            console.error('Failed to load auctions:', err);
        } finally {
            setLoading(false);
        }
    };

    const openEditDialog = (auction: Auction) => {
        setEditTarget(auction);
        setEditName(auction.name);
        setDeleteConfirm(false);
    };

    const closeEditDialog = () => {
        setEditTarget(null);
        setEditName('');
        setDeleteConfirm(false);
    };

    const handleRename = async () => {
        if (!editTarget || !editName.trim()) return;
        setEditSaving(true);
        try {
            await api.renameAuction(editTarget.id, editName.trim());
            toast.success('Auction renamed successfully');
            closeEditDialog();
            loadAuctions();
        } catch (err) {
            toast.error('Failed to rename auction');
        } finally {
            setEditSaving(false);
        }
    };

    const handleDelete = async () => {
        if (!editTarget) return;
        if (!deleteConfirm) {
            setDeleteConfirm(true);
            return;
        }
        setEditSaving(true);
        try {
            await api.deleteAuction(editTarget.id);
            toast.success(`Auction "${editTarget.name}" deleted`);
            closeEditDialog();
            loadAuctions();
        } catch (err) {
            toast.error('Failed to delete auction');
        } finally {
            setEditSaving(false);
        }
    };

    const handleExport = async (auction: Auction) => {
        try {
            // Fetch all items for this auction
            const allItems = await api.getInventoryItems();
            const auctionItems = allItems.filter(item => item.auction_id === auction.id);

            if (auctionItems.length === 0) {
                toast.error('No items found for this auction');
                return;
            }

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
                    'cost': (cost_pct / 100),
                    'cost price': { t: 'n', f: `ROUND(F${r}*H${r}, 2)` },
                    'Retail price': { t: 'n', f: `F${r}` },
                    '% min pr (+10%)': { t: 'n', f: `ROUND(F${r}*0.10, 2)` },
                    'min price': { t: 'n', f: `CEILING(I${r}+K${r}, 1)` }
                };
            });

            const worksheet = XLSX.utils.json_to_sheet(formattedData);
            const workbook = XLSX.utils.book_new();
            XLSX.utils.book_append_sheet(workbook, worksheet, 'Sheet1');
            const excelBuffer = XLSX.write(workbook, { bookType: 'xlsx', type: 'array' });

            const defaultName = `${auction.name.replace(/\s+/g, '_')}_Manager_Report.xlsx`;
            const savePath = await api.saveFile(defaultName);
            if (!savePath) return;

            await api.saveBinaryFile(savePath as string, new Uint8Array(excelBuffer));
            toast.success(`Exported ${auction.name} to Excel`);
        } catch (err) {
            console.error('Export failed:', err);
            toast.error('Export failed');
        }
    };

    const getStatusColor = (status: string) => {
        switch (status) {
            case 'Active': return 'bg-blue-500/15 text-blue-700 dark:text-blue-300';
            case 'Completed': return 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300';
            default: return 'bg-gray-500/15 text-gray-700 dark:text-gray-300';
        }
    };

    return (
        <div className="space-y-8 animate-fade-in">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Auctions</h1>
                    <p className="text-muted-foreground mt-1">
                        Manage your HiBid auctions
                    </p>
                </div>
                <div className="flex gap-2">
                    <Button onClick={() => setIsDialogOpen(true)}>
                        <Plus className="mr-2 h-4 w-4" />
                        New Auction
                    </Button>
                </div>
            </div>

            <CreateAuctionDialog
                open={isDialogOpen}
                onOpenChange={setIsDialogOpen}
                onSuccess={loadAuctions}
            />

            {/* Edit Auction Dialog */}
            <Dialog open={!!editTarget} onOpenChange={(open) => !open && closeEditDialog()}>
                <DialogContent className="sm:max-w-md">
                    <DialogHeader>
                        <DialogTitle>Edit Auction</DialogTitle>
                        <DialogDescription>
                            Rename the auction or delete it permanently.
                        </DialogDescription>
                    </DialogHeader>

                    <div className="space-y-4 py-2">
                        <div className="space-y-2">
                            <label className="text-sm font-medium">Auction Name</label>
                            <Input
                                value={editName}
                                onChange={(e) => setEditName(e.target.value)}
                                onKeyDown={(e) => e.key === 'Enter' && handleRename()}
                                placeholder="Enter auction name"
                            />
                        </div>
                    </div>

                    <DialogFooter className="flex-col sm:flex-row gap-2">
                        {deleteConfirm ? (
                            <div className="flex gap-2 w-full">
                                <Button
                                    variant="destructive"
                                    className="flex-1"
                                    onClick={handleDelete}
                                    disabled={editSaving}
                                >
                                    <Trash2 className="mr-2 h-4 w-4" />
                                    Confirm Delete
                                </Button>
                                <Button
                                    variant="outline"
                                    onClick={() => setDeleteConfirm(false)}
                                    disabled={editSaving}
                                >
                                    Cancel
                                </Button>
                            </div>
                        ) : (
                            <div className="flex gap-2 w-full">
                                <Button
                                    variant="destructive"
                                    size="icon"
                                    onClick={handleDelete}
                                    disabled={editSaving}
                                    title="Delete auction"
                                >
                                    <Trash2 className="h-4 w-4" />
                                </Button>
                                <div className="flex-1" />
                                <Button variant="outline" onClick={closeEditDialog} disabled={editSaving}>
                                    Cancel
                                </Button>
                                <Button
                                    onClick={handleRename}
                                    disabled={editSaving || !editName.trim() || editName.trim() === editTarget?.name}
                                >
                                    Save
                                </Button>
                            </div>
                        )}
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {auctions.length > 0 && (
                <div className="grid gap-4 md:grid-cols-2">
                    <Card>
                        <CardContent className="pt-6">
                            <div className="text-2xl font-bold text-blue-600">
                                {auctions.filter(a => a.status === 'Active').length}
                            </div>
                            <p className="text-sm text-muted-foreground">Active</p>
                        </CardContent>
                    </Card>
                    <Card>
                        <CardContent className="pt-6">
                            <div className="text-2xl font-bold text-emerald-600">
                                {auctions.filter(a => a.status === 'Completed').length}
                            </div>
                            <p className="text-sm text-muted-foreground">Completed</p>
                        </CardContent>
                    </Card>
                </div>
            )}

            <Card>
                <CardHeader>
                    <CardTitle>Auction List</CardTitle>
                    <CardDescription>
                        All auctions created in the system
                    </CardDescription>
                </CardHeader>
                <CardContent>
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>Name</TableHead>
                                <TableHead>Status</TableHead>
                                <TableHead>Lots</TableHead>
                                <TableHead>Start Date</TableHead>
                                <TableHead>Created</TableHead>
                                <TableHead className="text-right">Actions</TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {loading ? (
                                <TableRow>
                                    <TableCell colSpan={6} className="h-24 text-center">Loading auctions...</TableCell>
                                </TableRow>
                            ) : auctions.length === 0 ? (
                                <TableRow>
                                    <TableCell colSpan={6} className="h-24 text-center">No auctions found.</TableCell>
                                </TableRow>
                            ) : (
                                auctions.map((auction) => (
                                    <TableRow
                                        key={auction.id}
                                        className="cursor-pointer hover:bg-muted/50 transition-colors"
                                        onClick={() => navigate(`/auctions/${auction.id}`)}
                                    >
                                        <TableCell className="font-medium">
                                            <div className="flex items-center gap-2">
                                                <Calendar className="h-4 w-4 text-muted-foreground" />
                                                {auction.name}
                                            </div>
                                        </TableCell>
                                        <TableCell>
                                            <Badge variant="secondary" className={getStatusColor(auction.status)}>
                                                {auction.status}
                                            </Badge>
                                        </TableCell>
                                        <TableCell>{auction.total_lots} items</TableCell>
                                        <TableCell>
                                            {auction.start_date ? formatDate(auction.start_date) : '-'}
                                        </TableCell>
                                        <TableCell className="text-muted-foreground text-sm">
                                            {formatDate(auction.created_at)}
                                        </TableCell>
                                        <TableCell className="text-right" onClick={(e) => e.stopPropagation()}>
                                            <div className="flex justify-end gap-2">
                                                <Button
                                                    variant="outline"
                                                    size="sm"
                                                    className="border-emerald-500 text-emerald-600 hover:bg-emerald-50 hover:text-emerald-700 w-9 px-0"
                                                    onClick={() => handleExport(auction)}
                                                    title="Export to Excel"
                                                >
                                                    <FileSpreadsheet className="h-4 w-4" />
                                                </Button>
                                                <Button
                                                    variant="outline"
                                                    size="sm"
                                                    className="w-9 px-0"
                                                    onClick={() => openEditDialog(auction)}
                                                    title="Edit auction"
                                                >
                                                    <Pencil className="h-4 w-4" />
                                                </Button>
                                            </div>
                                        </TableCell>
                                    </TableRow>
                                ))
                            )}
                        </TableBody>
                    </Table>
                </CardContent>
            </Card>
        </div>
    );
}
