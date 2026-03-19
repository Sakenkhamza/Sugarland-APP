import { useEffect, useMemo, useState } from 'react';
import * as XLSX from 'xlsx';
import { BarChart3, CalendarRange, Download, TrendingUp, FileSpreadsheet, FolderOpen } from 'lucide-react';
import { api } from '@/lib/api';
import type { AuctionSummary, ProfitLossReport, VendorBreakdown, AuctionReport } from '@/types';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { formatCurrency, formatDate, formatNumber } from '@/lib/utils';
import { toast } from 'sonner';
import {
    Bar,
    BarChart,
    CartesianGrid,
    Legend,
    ResponsiveContainer,
    Tooltip,
    XAxis,
    YAxis,
} from 'recharts';

type ReportPeriod = 'week' | 'month' | 'all' | 'custom';

const PERIODS: Array<{ label: string; value: ReportPeriod }> = [
    { label: 'This Week', value: 'week' },
    { label: 'This Month', value: 'month' },
    { label: 'All Time', value: 'all' },
    { label: 'Custom', value: 'custom' },
];

function todayIso(): string {
    return new Date().toISOString().slice(0, 10);
}

function daysAgoIso(days: number): string {
    const date = new Date();
    date.setDate(date.getDate() - days);
    return date.toISOString().slice(0, 10);
}

export function ReportsPage() {
    const [period, setPeriod] = useState<ReportPeriod>('month');
    const [dateFrom, setDateFrom] = useState(daysAgoIso(29));
    const [dateTo, setDateTo] = useState(todayIso());

    const [plReport, setPlReport] = useState<ProfitLossReport | null>(null);
    const [auctionSummaries, setAuctionSummaries] = useState<AuctionSummary[]>([]);
    const [vendorBreakdown, setVendorBreakdown] = useState<VendorBreakdown[]>([]);
    const [auctionReports, setAuctionReports] = useState<AuctionReport[]>([]);
    const [loading, setLoading] = useState(true);
    const [exporting, setExporting] = useState(false);

    const shouldSkipCustomLoad = period === 'custom' && (!dateFrom || !dateTo);

    useEffect(() => {
        const load = async () => {
            if (shouldSkipCustomLoad) {
                setLoading(false);
                return;
            }

            try {
                setLoading(true);
                const from = period === 'custom' ? dateFrom : undefined;
                const to = period === 'custom' ? dateTo : undefined;
                const [pl, auctions, vendors] = await Promise.all([
                    api.getPLReportFiltered(period, from, to),
                    api.getAuctionSummaries(period, from, to),
                    api.getVendorBreakdown(period, from, to),
                ]);
                setPlReport(pl);
                setAuctionSummaries(auctions);
                setVendorBreakdown(vendors);
            } catch (error) {
                console.error('Failed to load reports:', error);
                toast.error('Failed to load reports');
            } finally {
                setLoading(false);
            }
        };

        load();
    }, [period, dateFrom, dateTo, shouldSkipCustomLoad]);

    useEffect(() => {
        api.getAllAuctionReports()
            .then(setAuctionReports)
            .catch((err) => console.error('Failed to load auction reports:', err));
    }, []);

    const auctionChartData = useMemo(
        () =>
            auctionSummaries.map((row) => ({
                shortName:
                    row.auction_name.length > 18
                        ? `${row.auction_name.slice(0, 18)}...`
                        : row.auction_name,
                fullName: row.auction_name,
                total_revenue: row.total_revenue,
                total_cogs: row.total_cogs,
                net_profit: row.net_profit,
                sold_count: row.sold_count,
                buyback_count: row.buyback_count,
                unsold_count: row.unsold_count,
            })),
        [auctionSummaries],
    );

    const vendorTotals = useMemo(() => {
        const totals = vendorBreakdown.reduce(
            (acc, row) => {
                acc.item_count += row.item_count;
                acc.total_retail += row.total_retail;
                acc.total_cost += row.total_cost;
                acc.total_revenue += row.total_revenue;
                acc.revenue_with_commission += row.revenue_with_commission;
                acc.profit_loss += row.profit_loss;
                return acc;
            },
            {
                item_count: 0,
                total_retail: 0,
                total_cost: 0,
                total_revenue: 0,
                revenue_with_commission: 0,
                profit_loss: 0,
                cost_pct: 0,
                revenue_pct: 0,
            },
        );
        totals.cost_pct =
            totals.total_retail > 0 ? (totals.total_cost / totals.total_retail) * 100 : 0;
        totals.revenue_pct =
            totals.total_retail > 0
                ? (totals.revenue_with_commission / totals.total_retail) * 100
                : 0;
        return totals;
    }, [vendorBreakdown]);

    const dateRangeLabel = useMemo(() => {
        if (period === 'custom') {
            return `${dateFrom || 'N/A'} - ${dateTo || 'N/A'}`;
        }
        if (period === 'week') {
            return `${daysAgoIso(6)} - ${todayIso()}`;
        }
        if (period === 'month') {
            return `${daysAgoIso(29)} - ${todayIso()}`;
        }
        return plReport?.period_label || 'All time';
    }, [period, dateFrom, dateTo, plReport?.period_label]);

    const handleExportSummaryExcel = async () => {
        if (!plReport) return;
        if (vendorBreakdown.length === 0) {
            toast.info('No vendor data to export');
            return;
        }

        try {
            setExporting(true);

            const wb = XLSX.utils.book_new();
            const ws: XLSX.WorkSheet = {};
            const headerRow = 3;
            const totalRow = 4;
            const buybackRow = 5;
            const unsoldRow = 6;
            const soldRow = 7;
            const vendorStartRow = 8;
            const vendorEndRow = vendorStartRow + vendorBreakdown.length - 1;
            const safeVendorEnd = Math.max(vendorStartRow, vendorEndRow);

            ws.A1 = {
                t: 's',
                v: `Отчет по продажам товаров на аукционе (${dateRangeLabel})`,
            };

            const headers = [
                'Vendor/Source',
                'Qty',
                'Retail Price',
                'Cost',
                'Cost %',
                'Sales',
                'Sales+15%',
                'Sales %',
                'P/L',
            ];
            headers.forEach((header, index) => {
                const col = XLSX.utils.encode_col(index);
                ws[`${col}${headerRow}`] = { t: 's', v: header };
            });

            ws[`A${totalRow}`] = { t: 's', v: 'Всего' };
            ws[`B${totalRow}`] = { t: 'n', v: plReport.total_lots };
            ws[`C${totalRow}`] = { f: `C${soldRow}`, t: 'n' };
            ws[`D${totalRow}`] = { f: `D${soldRow}`, t: 'n' };
            ws[`E${totalRow}`] = { f: `IF(C${totalRow}=0,0,D${totalRow}/C${totalRow})`, t: 'n' };
            ws[`F${totalRow}`] = { f: `F${soldRow}`, t: 'n' };
            ws[`G${totalRow}`] = { f: `G${soldRow}`, t: 'n' };
            ws[`H${totalRow}`] = { f: `IF(C${totalRow}=0,0,G${totalRow}/C${totalRow})`, t: 'n' };
            ws[`I${totalRow}`] = { f: `I${soldRow}`, t: 'n' };

            ws[`A${buybackRow}`] = { t: 's', v: 'Выкуплено обратно' };
            ws[`B${buybackRow}`] = { t: 'n', v: plReport.buyback_count };
            ws[`A${unsoldRow}`] = { t: 's', v: 'Не продано' };
            ws[`B${unsoldRow}`] = { t: 'n', v: plReport.unsold_count };

            ws[`A${soldRow}`] = { t: 's', v: 'Продано' };
            ws[`B${soldRow}`] = { t: 'n', v: plReport.sold_items };
            ws[`C${soldRow}`] = { f: `SUM(C${vendorStartRow}:C${safeVendorEnd})`, t: 'n' };
            ws[`D${soldRow}`] = { f: `SUM(D${vendorStartRow}:D${safeVendorEnd})`, t: 'n' };
            ws[`E${soldRow}`] = { f: `IF(C${soldRow}=0,0,D${soldRow}/C${soldRow})`, t: 'n' };
            ws[`F${soldRow}`] = { f: `SUM(F${vendorStartRow}:F${safeVendorEnd})`, t: 'n' };
            ws[`G${soldRow}`] = { f: `SUM(G${vendorStartRow}:G${safeVendorEnd})`, t: 'n' };
            ws[`H${soldRow}`] = { f: `IF(C${soldRow}=0,0,G${soldRow}/C${soldRow})`, t: 'n' };
            ws[`I${soldRow}`] = { f: `SUM(I${vendorStartRow}:I${safeVendorEnd})`, t: 'n' };

            vendorBreakdown.forEach((row, index) => {
                const r = vendorStartRow + index;
                ws[`A${r}`] = { t: 's', v: row.source || 'Unknown' };
                ws[`B${r}`] = { t: 'n', v: row.item_count };
                ws[`C${r}`] = { t: 'n', v: row.total_retail };
                ws[`D${r}`] = { t: 'n', v: row.total_cost };
                ws[`E${r}`] = { f: `IF(C${r}=0,0,D${r}/C${r})`, t: 'n' };
                ws[`F${r}`] = { t: 'n', v: row.total_revenue };
                ws[`G${r}`] = { f: `F${r}*1.15`, t: 'n' };
                ws[`H${r}`] = { f: `IF(C${r}=0,0,G${r}/C${r})`, t: 'n' };
                ws[`I${r}`] = { f: `G${r}-D${r}`, t: 'n' };
            });

            const lastRow = Math.max(vendorEndRow, soldRow);
            ws['!ref'] = `A1:I${lastRow}`;
            ws['!cols'] = [
                { wch: 28 },
                { wch: 8 },
                { wch: 15 },
                { wch: 12 },
                { wch: 10 },
                { wch: 12 },
                { wch: 12 },
                { wch: 10 },
                { wch: 12 },
            ];

            XLSX.utils.book_append_sheet(wb, ws, 'Summary');

            const defaultName =
                period === 'custom' && dateFrom && dateTo
                    ? `сводный_отчет_Хайбид_${dateFrom}_${dateTo}.xlsx`
                    : 'сводный_отчет_Хайбид.xlsx';
            const savePath = await api.saveFile(defaultName);
            if (!savePath) return;

            const buffer = XLSX.write(wb, { bookType: 'xlsx', type: 'array' });
            await api.saveBinaryFile(savePath as string, new Uint8Array(buffer));
            toast.success(`Summary Excel exported to ${savePath}`);
        } catch (error) {
            console.error('Failed to export summary excel:', error);
            toast.error('Failed to export summary Excel');
        } finally {
            setExporting(false);
        }
    };

    const handleOpenReport = async (filePath: string) => {
        try {
            await api.openReportFile(filePath);
        } catch {
            toast.error('Failed to open report file');
        }
    };

    if (loading) {
        return (
            <div className="flex h-96 items-center justify-center">
                <div className="text-center space-y-2">
                    <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent mx-auto" />
                    <p className="text-muted-foreground">Loading report data...</p>
                </div>
            </div>
        );
    }

    return (
        <div className="space-y-8 animate-fade-in">
            <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Reports</h1>
                    <p className="text-muted-foreground mt-1">
                        Financial analytics by period, auction, and vendor
                    </p>
                </div>
                <Button
                    onClick={handleExportSummaryExcel}
                    disabled={!plReport || exporting || vendorBreakdown.length === 0}
                    className="bg-emerald-600 hover:bg-emerald-700"
                >
                    <Download className="mr-2 h-4 w-4" />
                    {exporting ? 'Exporting...' : 'Export Summary Excel'}
                </Button>
            </div>

            <Card>
                <CardHeader className="pb-3">
                    <CardTitle className="text-base flex items-center gap-2">
                        <CalendarRange className="h-4 w-4" />
                        Period
                    </CardTitle>
                    <CardDescription>Select a time window for all report sections</CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div className="flex flex-wrap gap-2">
                        {PERIODS.map((item) => (
                            <Button
                                key={item.value}
                                variant={period === item.value ? 'default' : 'outline'}
                                size="sm"
                                onClick={() => setPeriod(item.value)}
                            >
                                {item.label}
                            </Button>
                        ))}
                    </div>
                    {period === 'custom' && (
                        <div className="grid gap-3 sm:grid-cols-2">
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground">From</p>
                                <Input
                                    type="date"
                                    value={dateFrom}
                                    onChange={(e) => setDateFrom(e.target.value)}
                                />
                            </div>
                            <div className="space-y-1">
                                <p className="text-xs text-muted-foreground">To</p>
                                <Input
                                    type="date"
                                    value={dateTo}
                                    onChange={(e) => setDateTo(e.target.value)}
                                />
                            </div>
                        </div>
                    )}
                </CardContent>
            </Card>

            {/* Auction Reports - generated Excel files */}
            {auctionReports.length > 0 && (() => {
                const byAuction = auctionReports.reduce<Record<string, { name: string; reports: AuctionReport[] }>>(
                    (acc, r) => {
                        if (!acc[r.auction_id]) acc[r.auction_id] = { name: r.auction_name, reports: [] };
                        acc[r.auction_id].reports.push(r);
                        return acc;
                    },
                    {},
                );
                return (
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
                                {Object.entries(byAuction).map(([auctionId, { name, reports }]) => (
                                    <div key={auctionId} className="rounded-lg border p-4 hover:bg-muted/30 transition-colors">
                                        <div className="flex items-center justify-between mb-3">
                                            <div className="flex items-center gap-2">
                                                <FolderOpen className="h-4 w-4 text-muted-foreground" />
                                                <span className="font-semibold">{name}</span>
                                            </div>
                                            <span className="text-xs text-muted-foreground">
                                                {reports[0] && formatDate(reports[0].created_at)}
                                            </span>
                                        </div>
                                        <div className="grid gap-2 sm:grid-cols-2">
                                            {reports.map((r) => (
                                                <Button
                                                    key={r.id}
                                                    variant="outline"
                                                    className="justify-start gap-2 h-auto py-3 px-4"
                                                    onClick={() => handleOpenReport(r.file_path)}
                                                >
                                                    <FileSpreadsheet className="h-4 w-4 text-emerald-600 shrink-0" />
                                                    <div className="text-left min-w-0">
                                                        <div className="font-medium text-sm truncate">
                                                            {r.report_type === 'detail'
                                                                ? 'Отчет (Detail Report)'
                                                                : 'Сводный отчет (Summary Report)'}
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
                );
            })()}

            {!plReport ? (
                <Card>
                    <CardContent className="py-12 text-center text-muted-foreground">
                        <BarChart3 className="h-10 w-10 mx-auto mb-3 opacity-30" />
                        No report data found for this period.
                    </CardContent>
                </Card>
            ) : (
                <>
                    <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                        <Card className="border-l-4 border-l-blue-500">
                            <CardHeader className="pb-2">
                                <CardTitle className="text-sm text-muted-foreground">Total Revenue</CardTitle>
                            </CardHeader>
                            <CardContent>
                                <div className="text-2xl font-bold">{formatCurrency(plReport.total_revenue)}</div>
                                <p className="text-xs text-muted-foreground mt-1">{plReport.period_label}</p>
                            </CardContent>
                        </Card>

                        <Card className="border-l-4 border-l-emerald-500">
                            <CardHeader className="pb-2">
                                <CardTitle className="text-sm text-muted-foreground">Net Profit</CardTitle>
                            </CardHeader>
                            <CardContent>
                                <div className="text-2xl font-bold text-emerald-700">{formatCurrency(plReport.net_profit)}</div>
                                <p className="text-xs text-muted-foreground mt-1">Margin: {plReport.margin_percent.toFixed(1)}%</p>
                            </CardContent>
                        </Card>

                        <Card className="border-l-4 border-l-amber-500">
                            <CardHeader className="pb-2">
                                <CardTitle className="text-sm text-muted-foreground">Sell-Through Rate</CardTitle>
                            </CardHeader>
                            <CardContent>
                                <div className="text-2xl font-bold">{plReport.sell_through_rate.toFixed(1)}%</div>
                                <p className="text-xs text-muted-foreground mt-1">Sold: {formatNumber(plReport.sold_items)}</p>
                            </CardContent>
                        </Card>

                        <Card className="border-l-4 border-l-indigo-500">
                            <CardHeader className="pb-2">
                                <CardTitle className="text-sm text-muted-foreground">Avg Sale Price</CardTitle>
                            </CardHeader>
                            <CardContent>
                                <div className="text-2xl font-bold">{formatCurrency(plReport.avg_sale_price)}</div>
                                <p className="text-xs text-muted-foreground mt-1">Lots: {formatNumber(plReport.total_lots)}</p>
                            </CardContent>
                        </Card>
                    </div>

                    <div className="grid gap-6 lg:grid-cols-2">
                        <Card>
                            <CardHeader>
                                <CardTitle>Revenue vs Cost vs Profit</CardTitle>
                                <CardDescription>By completed auction</CardDescription>
                            </CardHeader>
                            <CardContent>
                                <ResponsiveContainer width="100%" height={320}>
                                    <BarChart data={auctionChartData}>
                                        <CartesianGrid strokeDasharray="3 3" className="stroke-border" />
                                        <XAxis dataKey="shortName" tick={{ fontSize: 11 }} />
                                        <YAxis
                                            tick={{ fontSize: 11 }}
                                            tickFormatter={(v) =>
                                                `$${Number(v).toLocaleString('en-US', {
                                                    maximumFractionDigits: 0,
                                                })}`
                                            }
                                        />
                                        <Tooltip
                                            labelFormatter={(_, payload) => payload?.[0]?.payload?.fullName || ''}
                                            formatter={(value: number) =>
                                                `$${value.toLocaleString('en-US', {
                                                    minimumFractionDigits: 2,
                                                    maximumFractionDigits: 2,
                                                })}`
                                            }
                                        />
                                        <Legend />
                                        <Bar dataKey="total_revenue" name="Revenue" fill="#7c3aed" />
                                        <Bar dataKey="total_cogs" name="Cost" fill="#dc2626" />
                                        <Bar dataKey="net_profit" name="Net Profit" fill="#16a34a" />
                                    </BarChart>
                                </ResponsiveContainer>
                            </CardContent>
                        </Card>

                        <Card>
                            <CardHeader>
                                <CardTitle>Sold / Buyback / Unsold</CardTitle>
                                <CardDescription>Status composition by auction</CardDescription>
                            </CardHeader>
                            <CardContent>
                                <ResponsiveContainer width="100%" height={320}>
                                    <BarChart data={auctionChartData} layout="vertical">
                                        <CartesianGrid strokeDasharray="3 3" className="stroke-border" />
                                        <XAxis type="number" tick={{ fontSize: 11 }} />
                                        <YAxis dataKey="shortName" type="category" tick={{ fontSize: 11 }} width={110} />
                                        <Tooltip />
                                        <Legend />
                                        <Bar dataKey="sold_count" name="Sold" stackId="status" fill="#16a34a" />
                                        <Bar dataKey="buyback_count" name="Buyback" stackId="status" fill="#f59e0b" />
                                        <Bar dataKey="unsold_count" name="Unsold" stackId="status" fill="#6b7280" />
                                    </BarChart>
                                </ResponsiveContainer>
                            </CardContent>
                        </Card>
                    </div>

                    <Card>
                        <CardHeader>
                            <CardTitle>Vendor Breakdown</CardTitle>
                            <CardDescription>
                                Source | Items | Retail | Cost | Cost% | Sales | Sales+15% | Sales% | P/L
                            </CardDescription>
                        </CardHeader>
                        <CardContent>
                            <div className="rounded-md border overflow-x-auto">
                                <table className="w-full text-sm">
                                    <thead className="bg-muted/50 text-muted-foreground">
                                        <tr>
                                            <th className="px-3 py-2 text-left">Source</th>
                                            <th className="px-3 py-2 text-right">Items</th>
                                            <th className="px-3 py-2 text-right">Retail</th>
                                            <th className="px-3 py-2 text-right">Cost</th>
                                            <th className="px-3 py-2 text-right">Cost %</th>
                                            <th className="px-3 py-2 text-right">Sales</th>
                                            <th className="px-3 py-2 text-right">+15%</th>
                                            <th className="px-3 py-2 text-right">Sales %</th>
                                            <th className="px-3 py-2 text-right">P/L</th>
                                        </tr>
                                    </thead>
                                    <tbody className="divide-y">
                                        {vendorBreakdown.length === 0 ? (
                                            <tr>
                                                <td colSpan={9} className="px-3 py-8 text-center text-muted-foreground">
                                                    No vendor data for selected period
                                                </td>
                                            </tr>
                                        ) : (
                                            vendorBreakdown.map((row) => (
                                                <tr key={row.source} className="hover:bg-muted/30">
                                                    <td className="px-3 py-2 font-medium">{row.source || 'Unknown'}</td>
                                                    <td className="px-3 py-2 text-right">{formatNumber(row.item_count)}</td>
                                                    <td className="px-3 py-2 text-right">{formatCurrency(row.total_retail)}</td>
                                                    <td className="px-3 py-2 text-right">{formatCurrency(row.total_cost)}</td>
                                                    <td className="px-3 py-2 text-right">{row.cost_pct.toFixed(1)}%</td>
                                                    <td className="px-3 py-2 text-right">{formatCurrency(row.total_revenue)}</td>
                                                    <td className="px-3 py-2 text-right">{formatCurrency(row.revenue_with_commission)}</td>
                                                    <td className="px-3 py-2 text-right">{row.revenue_pct.toFixed(1)}%</td>
                                                    <td className={`px-3 py-2 text-right font-semibold ${row.profit_loss >= 0 ? 'text-emerald-600' : 'text-red-600'}`}>
                                                        {formatCurrency(row.profit_loss)}
                                                    </td>
                                                </tr>
                                            ))
                                        )}
                                        {vendorBreakdown.length > 0 && (
                                            <tr className="bg-muted/30 font-bold">
                                                <td className="px-3 py-2">TOTAL</td>
                                                <td className="px-3 py-2 text-right">{formatNumber(vendorTotals.item_count)}</td>
                                                <td className="px-3 py-2 text-right">{formatCurrency(vendorTotals.total_retail)}</td>
                                                <td className="px-3 py-2 text-right">{formatCurrency(vendorTotals.total_cost)}</td>
                                                <td className="px-3 py-2 text-right">{vendorTotals.cost_pct.toFixed(1)}%</td>
                                                <td className="px-3 py-2 text-right">{formatCurrency(vendorTotals.total_revenue)}</td>
                                                <td className="px-3 py-2 text-right">{formatCurrency(vendorTotals.revenue_with_commission)}</td>
                                                <td className="px-3 py-2 text-right">{vendorTotals.revenue_pct.toFixed(1)}%</td>
                                                <td className={`px-3 py-2 text-right ${vendorTotals.profit_loss >= 0 ? 'text-emerald-700' : 'text-red-700'}`}>
                                                    {formatCurrency(vendorTotals.profit_loss)}
                                                </td>
                                            </tr>
                                        )}
                                    </tbody>
                                </table>
                            </div>
                        </CardContent>
                    </Card>

                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <TrendingUp className="h-4 w-4" />
                                Auction History
                            </CardTitle>
                            <CardDescription>
                                Auction | Date | Lots | Sold | Buyback | Unsold | Revenue | Commission | Net Profit | Margin
                            </CardDescription>
                        </CardHeader>
                        <CardContent>
                            <div className="rounded-md border overflow-x-auto">
                                <table className="w-full text-sm">
                                    <thead className="bg-muted/50 text-muted-foreground">
                                        <tr>
                                            <th className="px-3 py-2 text-left">Auction</th>
                                            <th className="px-3 py-2 text-left">Date</th>
                                            <th className="px-3 py-2 text-right">Lots</th>
                                            <th className="px-3 py-2 text-right">Sold</th>
                                            <th className="px-3 py-2 text-right">Buyback</th>
                                            <th className="px-3 py-2 text-right">Unsold</th>
                                            <th className="px-3 py-2 text-right">Revenue</th>
                                            <th className="px-3 py-2 text-right">Commission</th>
                                            <th className="px-3 py-2 text-right">Net Profit</th>
                                            <th className="px-3 py-2 text-right">Margin</th>
                                        </tr>
                                    </thead>
                                    <tbody className="divide-y">
                                        {auctionSummaries.length === 0 ? (
                                            <tr>
                                                <td colSpan={10} className="px-3 py-8 text-center text-muted-foreground">
                                                    No completed auctions in this period
                                                </td>
                                            </tr>
                                        ) : (
                                            auctionSummaries.map((row) => (
                                                <tr key={row.auction_id} className="hover:bg-muted/30">
                                                    <td className="px-3 py-2 font-medium">{row.auction_name}</td>
                                                    <td className="px-3 py-2">{formatDate(row.completed_at)}</td>
                                                    <td className="px-3 py-2 text-right">{formatNumber(row.total_lots)}</td>
                                                    <td className="px-3 py-2 text-right">{formatNumber(row.sold_count)}</td>
                                                    <td className="px-3 py-2 text-right text-amber-600">{formatNumber(row.buyback_count)}</td>
                                                    <td className="px-3 py-2 text-right text-slate-600">{formatNumber(row.unsold_count)}</td>
                                                    <td className="px-3 py-2 text-right">{formatCurrency(row.total_revenue)}</td>
                                                    <td className="px-3 py-2 text-right">{formatCurrency(row.total_commission)}</td>
                                                    <td className="px-3 py-2 text-right font-medium text-emerald-600">{formatCurrency(row.net_profit)}</td>
                                                    <td className="px-3 py-2 text-right">{row.margin_percent.toFixed(1)}%</td>
                                                </tr>
                                            ))
                                        )}
                                    </tbody>
                                </table>
                            </div>
                        </CardContent>
                    </Card>
                </>
            )}
        </div>
    );
}
