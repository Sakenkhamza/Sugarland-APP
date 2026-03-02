import { useEffect, useState } from 'react';
import { api } from '@/lib/api';
import type { ProfitLossReport, AuctionPnlRow, AuctionReport } from '@/types';
import {
    BarChart3,
    DollarSign,
    TrendingUp,
    ArrowUpRight,
    Download,
    FileSpreadsheet,
    FolderOpen
} from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { formatCurrency, formatNumber, formatDate } from '@/lib/utils';
import { ResponsiveContainer, BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, Legend } from 'recharts';
import { toast } from 'sonner';

export function ReportsPage() {
    const [report, setReport] = useState<ProfitLossReport | null>(null);
    const [pnlList, setPnlList] = useState<AuctionPnlRow[]>([]);
    const [auctionReports, setAuctionReports] = useState<AuctionReport[]>([]);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        loadReport();
    }, []);

    const loadReport = async () => {
        try {
            setLoading(true);
            const [data, pnlData, reportsData] = await Promise.all([
                api.getPlReport(),
                api.getAuctionPnlList(),
                api.getAllAuctionReports()
            ]);
            setReport(data);
            setPnlList(pnlData);
            setAuctionReports(reportsData);
        } catch (err) {
            console.error('Failed to load report:', err);
        } finally {
            setLoading(false);
        }
    };

    const handleOpenReport = async (filePath: string) => {
        try {
            await api.openReportFile(filePath);
        } catch (err) {
            console.error('Failed to open report:', err);
            toast.error('Failed to open report file');
        }
    };

    if (loading) {
        return (
            <div className="flex h-96 items-center justify-center">
                <div className="text-center space-y-2">
                    <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent mx-auto" />
                    <p className="text-muted-foreground">Loading Reports...</p>
                </div>
            </div>
        );
    }

    // Group reports by auction
    const reportsByAuction = auctionReports.reduce<Record<string, { name: string; reports: AuctionReport[] }>>((acc, r) => {
        if (!acc[r.auction_id]) {
            acc[r.auction_id] = { name: r.auction_name, reports: [] };
        }
        acc[r.auction_id].reports.push(r);
        return acc;
    }, {});

    const hasFinancialData = report && report.sold_items > 0;
    const hasAuctionReports = auctionReports.length > 0;

    if (!hasFinancialData && !hasAuctionReports) {
        return (
            <div className="flex flex-col items-center justify-center h-64 text-muted-foreground mt-12">
                <BarChart3 className="h-12 w-12 mb-3 opacity-20" />
                <p className="font-medium">No reports available yet</p>
                <p className="text-sm">Complete an auction to generate reports</p>
            </div>
        );
    }

    const chartData = pnlList.map(row => ({
        name: row.auction_name.length > 14 ? row.auction_name.slice(0, 14) + '…' : row.auction_name,
        Revenue: Math.round(row.total_revenue),
        Cost: Math.round(row.total_cost),
        Profit: Math.round(row.net_profit),
    }));

    const compositionData = pnlList.map(row => ({
        name: row.auction_name.length > 14 ? row.auction_name.slice(0, 14) + '…' : row.auction_name,
        Sold: row.sold_items,
        Buyback: row.buyback_items,
        'Sell-through': row.total_items > 0
            ? Math.round((row.sold_items / row.total_items) * 100)
            : 0,
    }));

    return (
        <div className="space-y-8 animate-fade-in">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Reports</h1>
                    <p className="text-muted-foreground mt-1">
                        Financial performance and auction reports
                    </p>
                </div>
            </div>

            {/* Auction Reports Section */}
            {hasAuctionReports && (
                <Card className="border-l-4 border-l-blue-500">
                    <CardHeader>
                        <CardTitle className="flex items-center gap-2">
                            <FileSpreadsheet className="h-5 w-5 text-blue-600" />
                            Auction Reports
                        </CardTitle>
                        <CardDescription>
                            Generated Excel reports for completed auctions
                        </CardDescription>
                    </CardHeader>
                    <CardContent>
                        <div className="space-y-4">
                            {Object.entries(reportsByAuction).map(([auctionId, { name, reports }]) => (
                                <div key={auctionId} className="rounded-lg border p-4 hover:bg-muted/30 transition-colors">
                                    <div className="flex items-center justify-between mb-3">
                                        <div className="flex items-center gap-2">
                                            <FolderOpen className="h-4 w-4 text-muted-foreground" />
                                            <span className="font-semibold">{name}</span>
                                            <Badge variant="secondary" className="bg-emerald-500/15 text-emerald-700 dark:text-emerald-300 text-xs">
                                                Completed
                                            </Badge>
                                        </div>
                                        <span className="text-xs text-muted-foreground">
                                            {reports[0] && formatDate(reports[0].created_at)}
                                        </span>
                                    </div>
                                    <div className="grid gap-2 sm:grid-cols-2">
                                        {reports.map(r => (
                                            <Button
                                                key={r.id}
                                                variant="outline"
                                                className="justify-start gap-2 h-auto py-3 px-4"
                                                onClick={() => handleOpenReport(r.file_path)}
                                            >
                                                <FileSpreadsheet className="h-4 w-4 text-emerald-600 shrink-0" />
                                                <div className="text-left min-w-0">
                                                    <div className="font-medium text-sm truncate">
                                                        {r.report_type === 'detail' ? 'Отчет (Detail Report)' : 'Сводный отчет (Summary Report)'}
                                                    </div>
                                                    <div className="text-xs text-muted-foreground truncate">
                                                        {r.file_name}
                                                    </div>
                                                </div>
                                                <Download className="h-3 w-3 ml-auto shrink-0 text-muted-foreground" />
                                            </Button>
                                        ))}
                                    </div>
                                </div>
                            ))}
                        </div>
                    </CardContent>
                </Card>
            )}

            {/* Financial KPI Cards */}
            {hasFinancialData && report && (
                <>
                    <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
                        <Card className="animate-fade-in border-l-4 border-l-primary">
                            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                <CardTitle className="text-sm font-medium text-muted-foreground">
                                    Total Revenue
                                </CardTitle>
                                <DollarSign className="h-4 w-4 text-primary" />
                            </CardHeader>
                            <CardContent>
                                <div className="text-2xl font-bold">{formatCurrency(report.total_revenue)}</div>
                                <p className="text-xs text-muted-foreground mt-1">
                                    Gross Sales
                                </p>
                            </CardContent>
                        </Card>

                        <Card className="animate-fade-in border-l-4 border-l-red-500 delay-100">
                            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                <CardTitle className="text-sm font-medium text-muted-foreground">
                                    Cost of Goods
                                </CardTitle>
                                <TrendingUp className="h-4 w-4 text-red-500" />
                            </CardHeader>
                            <CardContent>
                                <div className="text-2xl font-bold">{formatCurrency(report.total_cogs)}</div>
                                <p className="text-xs text-muted-foreground mt-1">
                                    Original item cost
                                </p>
                            </CardContent>
                        </Card>

                        <Card className="animate-fade-in border-l-4 border-l-amber-500 delay-200">
                            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                <CardTitle className="text-sm font-medium text-muted-foreground">
                                    Expenses
                                </CardTitle>
                                <ArrowUpRight className="h-4 w-4 text-amber-500" />
                            </CardHeader>
                            <CardContent>
                                <div className="text-2xl font-bold">{formatCurrency(report.total_expenses)}</div>
                                <p className="text-xs text-muted-foreground mt-1">
                                    Fees &amp; Commissions
                                </p>
                            </CardContent>
                        </Card>

                        <Card className="animate-fade-in border-l-4 border-l-emerald-500 delay-300 bg-emerald-50/30">
                            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                                <CardTitle className="text-sm font-medium text-emerald-800">
                                    Net Profit
                                </CardTitle>
                                <DollarSign className="h-4 w-4 text-emerald-600" />
                            </CardHeader>
                            <CardContent>
                                <div className="text-2xl font-bold text-emerald-700">{formatCurrency(report.net_profit)}</div>
                                <p className="text-xs text-emerald-600/80 mt-1 font-medium">
                                    {report.margin_percent.toFixed(1)}% Margin
                                </p>
                            </CardContent>
                        </Card>
                    </div>

                    <div className="grid gap-6 md:grid-cols-2">
                        <Card className="col-span-1">
                            <CardHeader>
                                <CardTitle>Profit Breakdown</CardTitle>
                                <CardDescription>Visualizing margin components</CardDescription>
                            </CardHeader>
                            <CardContent>
                                <ResponsiveContainer width="100%" height={240}>
                                    <BarChart data={chartData} margin={{ top: 5, right: 10, left: 0, bottom: 5 }}>
                                        <CartesianGrid strokeDasharray="3 3" className="stroke-border" />
                                        <XAxis dataKey="name" tick={{ fontSize: 11 }} />
                                        <YAxis tick={{ fontSize: 11 }} tickFormatter={(v) => `$${(v / 1000).toFixed(0)}k`} />
                                        <Tooltip formatter={(value: number) => `$${value.toLocaleString()}`} />
                                        <Legend />
                                        <Bar dataKey="Revenue" fill="hsl(262 83% 58%)" radius={[3, 3, 0, 0]} />
                                        <Bar dataKey="Cost" fill="hsl(0 84% 60%)" radius={[3, 3, 0, 0]} />
                                        <Bar dataKey="Profit" fill="hsl(142 76% 36%)" radius={[3, 3, 0, 0]} />
                                    </BarChart>
                                </ResponsiveContainer>
                            </CardContent>
                        </Card>

                        <Card className="col-span-1">
                            <CardHeader>
                                <CardTitle>Sold vs Buyback per Auction</CardTitle>
                                <CardDescription>Auction volume composition</CardDescription>
                            </CardHeader>
                            <CardContent>
                                <ResponsiveContainer width="100%" height={240}>
                                    <BarChart layout="vertical" data={compositionData} margin={{ top: 5, right: 10, left: 10, bottom: 5 }}>
                                        <CartesianGrid strokeDasharray="3 3" className="stroke-border" />
                                        <XAxis type="number" tick={{ fontSize: 11 }} />
                                        <YAxis dataKey="name" type="category" tick={{ fontSize: 11 }} width={80} />
                                        <Tooltip />
                                        <Legend />
                                        <Bar dataKey="Sold" stackId="a" fill="hsl(142 76% 36%)" radius={[0, 0, 0, 0]} />
                                        <Bar dataKey="Buyback" stackId="a" fill="hsl(35 91% 54%)" radius={[0, 3, 3, 0]} />
                                    </BarChart>
                                </ResponsiveContainer>
                            </CardContent>
                        </Card>
                    </div>

                    <Card>
                        <CardHeader>
                            <CardTitle>Per-Auction Breakdown</CardTitle>
                            <CardDescription>Detailed metrics for recent completed auctions</CardDescription>
                        </CardHeader>
                        <CardContent>
                            <div className="rounded-md border overflow-x-auto">
                                <table className="w-full text-sm text-left">
                                    <thead className="bg-muted/50 text-muted-foreground font-medium">
                                        <tr>
                                            <th className="px-4 py-3">Auction</th>
                                            <th className="px-4 py-3 text-right">Sold</th>
                                            <th className="px-4 py-3 text-right">Buyback</th>
                                            <th className="px-4 py-3 text-right">Revenue</th>
                                            <th className="px-4 py-3 text-right">Cost</th>
                                            <th className="px-4 py-3 text-right">Commission</th>
                                            <th className="px-4 py-3 text-right">Net Profit</th>
                                            <th className="px-4 py-3 text-right">Margin</th>
                                        </tr>
                                    </thead>
                                    <tbody className="divide-y">
                                        {pnlList.length === 0 ? (
                                            <tr>
                                                <td colSpan={8} className="px-4 py-8 text-center text-muted-foreground">
                                                    No completed auctions yet
                                                </td>
                                            </tr>
                                        ) : (
                                            pnlList.map((row) => {
                                                const margin = row.total_revenue > 0 ? ((row.net_profit / row.total_revenue) * 100).toFixed(1) + '%' : '0.0%';
                                                return (
                                                    <tr key={row.auction_id} className="hover:bg-muted/30">
                                                        <td className="px-4 py-3 font-medium">{row.auction_name}</td>
                                                        <td className="px-4 py-3 text-right">{row.sold_items}</td>
                                                        <td className="px-4 py-3 text-right text-amber-600 font-medium">{row.buyback_items}</td>
                                                        <td className="px-4 py-3 text-right">{formatCurrency(row.total_revenue)}</td>
                                                        <td className="px-4 py-3 text-right text-red-600">{formatCurrency(row.total_cost)}</td>
                                                        <td className="px-4 py-3 text-right">{formatCurrency(row.total_commission)}</td>
                                                        <td className="px-4 py-3 text-right font-medium text-emerald-600">{formatCurrency(row.net_profit)}</td>
                                                        <td className="px-4 py-3 text-right font-medium">{margin}</td>
                                                    </tr>
                                                );
                                            })
                                        )}
                                    </tbody>
                                </table>
                            </div>
                        </CardContent>
                    </Card>

                    <div className="flex justify-end text-sm text-muted-foreground">
                        Report generated for {formatNumber(report.sold_items)} sold items
                    </div>
                </>
            )}
        </div>
    );
}
