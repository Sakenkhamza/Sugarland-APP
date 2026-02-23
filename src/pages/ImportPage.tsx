import { useState } from 'react';
import { api } from '@/lib/api';
import type { ManifestSummary, ValidationResult } from '@/types';
import { Upload, Check, AlertCircle } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Progress } from '@/components/ui/progress';
import { formatCurrency } from '@/lib/utils';

export function ImportPage() {
    const navigate = useNavigate();
    const [file, setFile] = useState<string | null>(null);
    const [importing, setImporting] = useState(false);
    const [progress, setProgress] = useState(0);
    const [result, setResult] = useState<ManifestSummary | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [validation, setValidation] = useState<ValidationResult | null>(null);
    const [validating, setValidating] = useState(false);

    const handleSelectFile = async () => {
        try {
            const selected = await api.selectFile([{ name: 'CSV', extensions: ['csv'] }]);
            if (selected && typeof selected === 'string') {
                setFile(selected);
                setResult(null);
                setError(null);
                setValidation(null);
                setProgress(0);

                setValidating(true);
                try {
                    const v = await api.validateCsv(selected);
                    setValidation(v);
                } catch {
                    // silently ignore to avoid blocking import
                } finally {
                    setValidating(false);
                }
            }
        } catch (err) {
            console.error('File selection failed:', err);
        }
    };

    const handleImport = async () => {
        if (!file) return;

        setImporting(true);
        setProgress(10); // Start progress

        try {
            // Simulate progress for UX
            const interval = setInterval(() => {
                setProgress(p => Math.min(p + 10, 90));
            }, 200);

            const summary = await api.importManifest(file);

            clearInterval(interval);
            setProgress(100);
            setResult(summary);
            setFile(null);
        } catch (err) {
            console.error('Import failed:', err);
            // setResult(null); // Keep previous result?
            setError('Failed to import manifest. Please check the file format.');
            // Simulating error result for now if needed
        } finally {
            setImporting(false);
        }
    };

    return (
        <div className="space-y-8 max-w-2xl mx-auto animate-fade-in">
            <div className="text-center">
                <h1 className="text-3xl font-bold tracking-tight">Import Manifest</h1>
                <p className="text-muted-foreground mt-2">
                    Upload a B-Stock CSV manifest to populate inventory
                </p>
            </div>

            <Card className="border-2 border-dashed">
                <CardContent className="flex flex-col items-center justify-center p-12 space-y-4">
                    <div className="p-4 bg-primary/10 rounded-full">
                        <Upload className="h-8 w-8 text-primary" />
                    </div>
                    <div className="text-center space-y-1">
                        <h3 className="font-semibold text-lg">
                            {file ? 'File Selected' : 'Select Manifest CSV'}
                        </h3>
                        <p className="text-sm text-muted-foreground">
                            {file ? file : 'Drag and drop or click to browse'}
                        </p>
                    </div>
                    <Button onClick={handleSelectFile} variant={file ? "outline" : "default"}>
                        {file ? "Change File" : "Browse Files"}
                    </Button>
                </CardContent>
            </Card>

            {validating && (
                <p className="text-sm text-muted-foreground text-center animate-pulse">Validating CSV format...</p>
            )}

            {validation && !validating && (
                <div className={`p-3 rounded-lg border text-sm flex items-start gap-2 ${validation.valid
                        ? "bg-emerald-50 border-emerald-200 text-emerald-700"
                        : "bg-red-50 border-red-200 text-red-700"
                    }`}>
                    {validation.valid ? <Check className="h-4 w-4 mt-0.5 shrink-0" /> : <AlertCircle className="h-4 w-4 mt-0.5 shrink-0" />}
                    <div>
                        <p className="font-medium">{validation.message}</p>
                        {validation.warnings.length > 0 && (
                            <ul className="mt-1 space-y-0.5 text-xs opacity-80">
                                {validation.warnings.map((w, i) => <li key={i}>⚠ {w}</li>)}
                            </ul>
                        )}
                    </div>
                </div>
            )}

            {file && !importing && !result && validation?.valid !== false && (
                <div className="flex justify-center animate-fade-in">
                    <Button size="lg" onClick={handleImport} className="w-full sm:w-auto">
                        Process Import
                    </Button>
                </div>
            )}

            {importing && (
                <Card className="animate-fade-in">
                    <CardHeader>
                        <CardTitle className="text-base">Processing Import...</CardTitle>
                        <CardDescription>Parsing CSV and calculating costs</CardDescription>
                    </CardHeader>
                    <CardContent className="space-y-2">
                        <Progress value={progress} />
                        <p className="text-xs text-muted-foreground text-center">{progress}% complete</p>
                    </CardContent>
                </Card>
            )}

            {error && (
                <div className="bg-red-50 text-red-700 p-4 rounded-lg flex items-center gap-3 animate-fade-in border border-red-200">
                    <AlertCircle className="h-5 w-5" />
                    <div>
                        <p className="font-medium">Import Error</p>
                        <p className="text-sm">{error}</p>
                    </div>
                </div>
            )}

            {result && (
                <Card className="bg-emerald-50/50 border-emerald-200 animate-fade-in">
                    <CardHeader>
                        <div className="flex items-center gap-2">
                            <div className="p-1 bg-emerald-100 rounded-full">
                                <Check className="h-4 w-4 text-emerald-600" />
                            </div>
                            <CardTitle className="text-emerald-700">Import Successful</CardTitle>
                        </div>
                        <CardDescription className="text-emerald-600/80">
                            Manifest processed and added to inventory
                        </CardDescription>
                    </CardHeader>
                    <CardContent>
                        <div className="grid grid-cols-3 gap-4 text-center">
                            <div className="p-3 bg-white/50 rounded-lg border border-emerald-100">
                                <div className="text-2xl font-bold text-emerald-700">{result.items_count}</div>
                                <div className="text-xs text-emerald-600">Items Added</div>
                            </div>
                            <div className="p-3 bg-white/50 rounded-lg border border-emerald-100">
                                <div className="text-lg font-bold text-emerald-700">{formatCurrency(result.total_retail)}</div>
                                <div className="text-xs text-emerald-600">Total Retail</div>
                            </div>
                            <div className="p-3 bg-white/50 rounded-lg border border-emerald-100">
                                <div className="text-lg font-bold text-emerald-700">{formatCurrency(result.total_cost)}</div>
                                <div className="text-xs text-emerald-600">Total Cost</div>
                            </div>
                        </div>
                        <div className="mt-6 flex justify-center gap-3">
                            <Button variant="outline" onClick={() => setFile(null)}>Import Another</Button>
                            <Button onClick={() => navigate('/inventory')}>View Inventory</Button>
                        </div>
                    </CardContent>
                </Card>
            )}
        </div>
    );
}
