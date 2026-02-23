import { useEffect, useState } from 'react';
import { api } from '@/lib/api';
import type { Auction } from '@/types';
import { Plus, Download, Calendar } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { useNavigate } from 'react-router-dom';
import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
} from '@/components/ui/table';
import { formatDate } from '@/lib/utils';
import { CreateAuctionDialog } from '@/components/auctions/CreateAuctionDialog';
import { toast } from 'sonner';

export function AuctionsPage() {
    const navigate = useNavigate();
    const [auctions, setAuctions] = useState<Auction[]>([]);
    const [loading, setLoading] = useState(true);
    const [isDialogOpen, setIsDialogOpen] = useState(false);

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
    const handleExport = async (auction: Auction) => {
        try {
            const defaultName = `${auction.name.replace(/\s+/g, '_')}_lots.csv`;
            const savePath = await api.saveFile(defaultName);
            if (!savePath) return;

            await api.exportAuctionCsv(auction.id, savePath as string);
            toast.success(`Exported ${auction.name} to CSV`);
        } catch (err) {
            console.error('Export failed:', err);
            toast.error('Export failed');
        }
    };

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

            {auctions.length > 0 && (
                <div className="grid gap-4 md:grid-cols-3">
                    <Card>
                        <CardContent className="pt-6">
                            <div className="text-2xl font-bold">
                                {auctions.filter(a => a.status === 'Draft').length}
                            </div>
                            <p className="text-sm text-muted-foreground">Draft</p>
                        </CardContent>
                    </Card>
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
                                                <Button variant="outline" size="sm" onClick={() => handleExport(auction)}>
                                                    <Download className="h-4 w-4 mr-1" />
                                                    CSV
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
