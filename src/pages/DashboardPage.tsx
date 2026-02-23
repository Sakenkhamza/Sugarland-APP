import { useEffect, useState } from 'react';
import { api } from '@/lib/api';
import type { DashboardStats } from '@/types';
import {
    DollarSign,
    Package,
    TrendingUp,
    AlertCircle
} from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

import { formatCurrency, formatNumber, formatDate } from '@/lib/utils';
import { useNavigate } from 'react-router-dom';
import type { Auction } from '@/types';
import { CreateAuctionDialog } from '@/components/auctions/CreateAuctionDialog';

export function DashboardPage() {
    const navigate = useNavigate();
    const [stats, setStats] = useState<DashboardStats | null>(null);
    const [loading, setLoading] = useState(true);

    // Add auctions state
    const [auctions, setAuctions] = useState<Auction[]>([]);
    const [loadingAuctions, setLoadingAuctions] = useState(true);
    const [isDialogOpen, setIsDialogOpen] = useState(false);

    useEffect(() => {
        loadData();
    }, []);

    const loadData = async () => {
        loadStats();
        loadAuctions();
    };

    const loadAuctions = async () => {
        try {
            setLoadingAuctions(true);
            const data = await api.getAuctions();
            setAuctions(data.slice(0, 5)); // top 5
        } catch (err) {
            console.error('Failed to load auctions:', err);
        } finally {
            setLoadingAuctions(false);
        }
    };

    const loadStats = async () => {
        try {
            setLoading(true);
            const data = await api.getDashboardStats();
            setStats(data);
        } catch (err) {
            console.error('Failed to load dashboard stats:', err);
        } finally {
            setLoading(false);
        }
    };

    if (loading) {
        return (
            <div className="flex h-96 items-center justify-center">
                <div className="flex flex-col items-center gap-4">
                    <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
                    <p className="text-muted-foreground">Loading dashboard...</p>
                </div>
            </div>
        );
    }

    // Fallback if stats fail to load
    const data = stats || {
        total_items: 0,
        in_stock: 0,
        listed: 0,
        sold: 0,
        buyback: 0,
        total_retail_value: 0,
        total_cost: 0,
        active_auctions: 0,
    };

    const grossMargin = data.total_retail_value > 0
        ? ((data.total_retail_value - data.total_cost) / data.total_retail_value) * 100
        : 0;

    return (
        <div className="space-y-8 animate-fade-in">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Dashboard</h1>
                    <p className="text-muted-foreground mt-1">
                        Overview of your liquidation business
                    </p>
                </div>
                <div className="flex gap-3">
                    <Button variant="outline" onClick={() => loadData()}>
                        Refresh
                    </Button>
                    <Button onClick={() => navigate('/import')}>
                        Import Manifest
                    </Button>
                </div>
            </div>

            {/* KPI Cards */}
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
                <Card className="animate-fade-in border-l-4 border-l-primary">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium text-muted-foreground">
                            Total Retail Value
                        </CardTitle>
                        <DollarSign className="h-4 w-4 text-primary" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{formatCurrency(data.total_retail_value)}</div>
                        <p className="text-xs text-muted-foreground mt-1">
                            {formatCurrency(data.total_cost)} total cost
                        </p>
                    </CardContent>
                </Card>

                <Card className="animate-fade-in border-l-4 border-l-blue-500 delay-100">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium text-muted-foreground">
                            Inventory Status
                        </CardTitle>
                        <Package className="h-4 w-4 text-blue-500" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{formatNumber(data.in_stock)}</div>
                        <p className="text-xs text-muted-foreground mt-1">
                            In Stock ({data.listed} listed)
                        </p>
                    </CardContent>
                </Card>

                <Card className="animate-fade-in border-l-4 border-l-emerald-500 delay-200">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium text-muted-foreground">
                            Est. Gross Margin
                        </CardTitle>
                        <TrendingUp className="h-4 w-4 text-emerald-500" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold text-emerald-600">
                            {grossMargin.toFixed(1)}%
                        </div>
                        <p className="text-xs text-muted-foreground mt-1">
                            Potential profit
                        </p>
                    </CardContent>
                </Card>

                <Card className="animate-fade-in border-l-4 border-l-amber-500 delay-300">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium text-muted-foreground">
                            Active Auctions
                        </CardTitle>
                        <AlertCircle className="h-4 w-4 text-amber-500" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{data.active_auctions}</div>
                        <p className="text-xs text-muted-foreground mt-1">
                            Auctions currently live
                        </p>
                    </CardContent>
                </Card>
            </div>

            {/* Auctions Widget */}
            <div className="w-full">
                <Card className="transition-all hover:shadow-md">
                    <CardHeader className="flex flex-row items-center justify-between">
                        <div>
                            <CardTitle>Auctions</CardTitle>
                            <CardDescription>
                                Active and recent auctions
                            </CardDescription>
                        </div>
                        <div className="flex gap-2">
                            <Button variant="outline" size="sm" onClick={() => navigate('/auctions')}>
                                View All
                            </Button>
                            <Button size="sm" onClick={() => setIsDialogOpen(true)}>
                                New Auction
                            </Button>
                        </div>
                    </CardHeader>
                    <CardContent>
                        {loadingAuctions ? (
                            <div className="flex h-24 items-center justify-center">
                                <div className="h-6 w-6 animate-spin rounded-full border-4 border-primary border-t-transparent" />
                            </div>
                        ) : auctions.length === 0 ? (
                            <div className="flex h-24 items-center justify-center text-muted-foreground">
                                No auctions yet
                            </div>
                        ) : (
                            <div className="space-y-4">
                                {auctions.map((auction) => (
                                    <div
                                        key={auction.id}
                                        className="flex items-center justify-between border-b pb-4 last:border-0 last:pb-0 cursor-pointer hover:bg-muted/50 p-2 rounded transition-colors -mx-2"
                                        onClick={() => navigate(`/auctions/${auction.id}`)}
                                    >
                                        <div className="flex flex-col gap-1">
                                            <span className="font-medium">{auction.name}</span>
                                            <span className="text-xs text-muted-foreground">
                                                Created on {formatDate(auction.created_at)}
                                            </span>
                                        </div>
                                        <div className="flex items-center gap-4">
                                            <div className="text-sm">
                                                <span className="font-medium">{auction.total_lots}</span>
                                                <span className="text-muted-foreground ml-1">Lots</span>
                                            </div>
                                            <Badge variant="secondary" className={
                                                auction.status === 'Active' ? 'bg-blue-500/15 text-blue-700 dark:text-blue-300' :
                                                    auction.status === 'Completed' ? 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300' :
                                                        auction.status === 'Cancelled' ? 'bg-red-500/15 text-red-700 dark:text-red-300' :
                                                            'bg-gray-500/15 text-gray-700 dark:text-gray-300'
                                            }>
                                                {auction.status}
                                            </Badge>
                                        </div>
                                    </div>
                                ))}
                            </div>
                        )}
                    </CardContent>
                </Card>
            </div>

            <CreateAuctionDialog
                open={isDialogOpen}
                onOpenChange={setIsDialogOpen}
                onSuccess={loadData}
            />
        </div>
    );
}
