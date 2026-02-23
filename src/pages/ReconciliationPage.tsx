import { useEffect, useState } from 'react';
import { api } from '@/lib/api';
import type { Auction, ReconciliationSummary } from '@/types';
import { FileText, CheckCircle2, AlertTriangle } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Progress } from '@/components/ui/progress';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { formatCurrency } from '@/lib/utils'; // Optional

export function ReconciliationPage() {
    const [auctions, setAuctions] = useState<Auction[]>([]);
    const [selectedAuctionId, setSelectedAuctionId] = useState('');
    const [file, setFile] = useState<string | null>(null);
    const [scanning, setScanning] = useState(false);
    const [progress, setProgress] = useState(0);
    const [results, setResults] = useState<ReconciliationSummary | null>(null);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        loadAuctions();
    }, []);

    const loadAuctions = async () => {
        try {
            const data = await api.getAuctions();
            const eligibleAuctions = data.filter(a => a.status === 'Active' || a.status === 'Completed');
            setAuctions(eligibleAuctions);
        } catch (err) {
            console.error('Failed to load auctions:', err);
        }
    };

    const handleSelectFile = async () => {
        try {
            const selected = await api.selectFile([{ name: 'CSV', extensions: ['csv'] }]);
            if (selected && typeof selected === 'string') {
                setFile(selected);
                setResults(null);
                setError(null);
            }
        } catch (err) {
            console.error('File select failed:', err);
        }
    };

    const handleReconcile = async () => {
        if (!selectedAuctionId || !file) return;

        setScanning(true);
        setResults(null);
        setError(null);
        setProgress(5);

        const interval = setInterval(() => {
            setProgress(p => p >= 85 ? 85 : p + Math.random() * 8);
        }, 400);

        try {
            const summary = await api.reconcileAuction(selectedAuctionId, file);
            setResults(summary);
            setProgress(100);
        } catch (err) {
            console.error('Reconciliation failed:', err);
            setError('Failed to process reconciliation. Please check the file format and auction selection.');
        } finally {
            clearInterval(interval);
            setTimeout(() => setScanning(false), 500);
        }
    };

    return (
        <div className="space-y-8 max-w-4xl mx-auto animate-fade-in">
            <div className="text-center">
                <h1 className="text-3xl font-bold tracking-tight">Reconciliation</h1>
                <p className="text-muted-foreground mt-2">
                    Upload HiBid results to close the loop on sold inventory
                </p>
            </div>

            <div className="grid gap-8 md:grid-cols-2">
                <Card>
                    <CardHeader>
                        <CardTitle>1. Select Auction</CardTitle>
                        <CardDescription>Choose the completed auction to reconcile</CardDescription>
                    </CardHeader>
                    <CardContent>
                        {auctions.length === 0 ? (
                            <p className="text-sm text-muted-foreground p-2">
                                No active or completed auctions found. Activate an auction first.
                            </p>
                        ) : (
                            <Select value={selectedAuctionId} onValueChange={setSelectedAuctionId}>
                                <SelectTrigger>
                                    <SelectValue placeholder="Select an auction..." />
                                </SelectTrigger>
                                <SelectContent>
                                    {auctions.map(a => (
                                        <SelectItem key={a.id} value={a.id}>
                                            {a.name} — {a.status} · {a.total_lots} {a.total_lots === 1 ? 'lot' : 'lots'}
                                        </SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                        )}
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader>
                        <CardTitle>2. Upload Results</CardTitle>
                        <CardDescription>Select the HiBid export CSV file</CardDescription>
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <div className="flex items-center gap-3 p-3 border rounded-md bg-muted/50">
                            <FileText className="h-5 w-5 text-muted-foreground" />
                            <span className="text-sm truncate flex-1">
                                {file ? file : 'No file selected'}
                            </span>
                            <Button variant="outline" size="sm" onClick={handleSelectFile}>
                                Browse
                            </Button>
                        </div>
                    </CardContent>
                </Card>
            </div>

            <div className="flex justify-center">
                <Button
                    size="lg"
                    disabled={!selectedAuctionId || !file || scanning}
                    onClick={handleReconcile}
                    className="w-full md:w-auto"
                >
                    {scanning ? 'Processing...' : 'Run Reconciliation'}
                </Button>
            </div>

            {scanning && (
                <Card className="animate-fade-in">
                    <CardContent className="py-6">
                        <div className="space-y-2">
                            <div className="flex justify-between text-sm">
                                <span>Matching lots...</span>
                                <span>Processing</span>
                            </div>
                            <Progress value={progress} className="transition-all" />
                        </div>
                    </CardContent>
                </Card>
            )}

            {error && (
                <div className="bg-red-50 text-red-700 p-4 rounded-lg flex items-center gap-3 animate-fade-in border border-red-200">
                    <AlertTriangle className="h-5 w-5" />
                    <p>{error}</p>
                </div>
            )}

            {results && (
                <Card className="bg-emerald-50/50 border-emerald-200 animate-fade-in shadow-lg">
                    <CardHeader>
                        <div className="flex items-center gap-2">
                            <CheckCircle2 className="h-6 w-6 text-emerald-600" />
                            <CardTitle className="text-emerald-800">Reconciliation Complete</CardTitle>
                        </div>
                    </CardHeader>
                    <CardContent>
                        <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-center mb-6">
                            <div className="p-4 bg-white rounded-lg shadow-sm">
                                <div className="text-3xl font-bold text-emerald-700">{results.sold_count}</div>
                                <div className="text-sm text-emerald-600">Items Sold</div>
                            </div>
                            <div className="p-4 bg-white rounded-lg shadow-sm">
                                <div className="text-3xl font-bold text-amber-600">{results.buyback_count}</div>
                                <div className="text-sm text-amber-600">Buybacks</div>
                            </div>
                            <div className="p-4 bg-white rounded-lg shadow-sm">
                                <div className="text-3xl font-bold text-emerald-700">{formatCurrency(results.total_revenue)}</div>
                                <div className="text-sm text-emerald-600">Total Revenue</div>
                            </div>
                            <div className="p-4 bg-white rounded-lg shadow-sm">
                                <div className="text-3xl font-bold text-blue-700">{formatCurrency(results.total_profit)}</div>
                                <div className="text-sm text-blue-600">Net Profit</div>
                            </div>
                        </div>

                        {results.errors && results.errors.length > 0 && (
                            <div className="mt-6 p-4 bg-white/80 rounded-lg border border-red-100">
                                <h4 className="font-semibold text-red-700 mb-2 flex items-center gap-2">
                                    <AlertTriangle className="h-4 w-4" />
                                    Exceptions ({results.errors.length})
                                </h4>
                                <ul className="text-sm text-red-600 space-y-1 max-h-32 overflow-y-auto">
                                    {results.errors.map((err, i) => (
                                        <li key={i}>• {err}</li>
                                    ))}
                                </ul>
                            </div>
                        )}
                    </CardContent>
                </Card>
            )}
        </div>
    );
}
