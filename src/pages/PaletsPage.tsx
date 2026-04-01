import { useState } from 'react';
import { FileUp, FileSpreadsheet, LoaderCircle } from 'lucide-react';
import { toast } from 'sonner';
import { api } from '@/lib/api';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';

export function PaletsPage() {
    const [isGenerating, setIsGenerating] = useState(false);
    const [lastCsvPath, setLastCsvPath] = useState<string | null>(null);
    const [lastOutputPath, setLastOutputPath] = useState<string | null>(null);

    const handleGenerateManifest = async () => {
        const filePath = await api.selectFile([{ name: 'CSV Files', extensions: ['csv'] }]);
        if (!filePath) {
            toast.info('CSV file selection was cancelled');
            return;
        }

        const savePath = await api.saveFile('Manifests for pallets (one by one).xlsx');
        if (!savePath) {
            toast.info('Excel export was cancelled');
            return;
        }

        setIsGenerating(true);
        setLastCsvPath(filePath as string);

        try {
            const result = await api.generatePalletManifestReport(filePath as string, savePath as string);
            setLastOutputPath(result.file_path);
            toast.success(`Excel generated: ${result.pallets_count} pallets, ${result.items_count} rows`);
        } catch (error) {
            console.error('Failed to generate pallet manifest report:', error);
            toast.error(`Failed to generate Excel: ${String(error)}`);
        } finally {
            setIsGenerating(false);
        }
    };

    return (
        <div className="space-y-8 animate-fade-in max-w-4xl">
            <div>
                <h1 className="text-3xl font-bold tracking-tight">Palets</h1>
                <p className="text-muted-foreground mt-1">
                    Import a pallet CSV manifest and generate the Excel workbook in the reference format
                </p>
            </div>

            <Card className="border-primary/20">
                <CardHeader>
                    <CardTitle className="text-lg flex items-center gap-2">
                        <FileSpreadsheet className="h-5 w-5 text-primary" />
                        Pallet Manifest Export
                    </CardTitle>
                    <CardDescription>
                        Supports B-Stock manifest CSV files such as the examples in `palet_ref`
                    </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    <Button
                        onClick={handleGenerateManifest}
                        disabled={isGenerating}
                        className="min-w-56"
                    >
                        {isGenerating ? (
                            <>
                                <LoaderCircle className="mr-2 h-4 w-4 animate-spin" />
                                Generating Excel...
                            </>
                        ) : (
                            <>
                                <FileUp className="mr-2 h-4 w-4" />
                                Upload CSV and Generate Excel
                            </>
                        )}
                    </Button>

                    {lastCsvPath && (
                        <div className="rounded-lg border bg-muted/20 p-4 space-y-2">
                            <p className="text-sm font-medium">Last imported CSV</p>
                            <p className="text-sm text-muted-foreground break-all">{lastCsvPath}</p>
                            {lastOutputPath && (
                                <>
                                    <p className="text-sm font-medium pt-2">Generated Excel</p>
                                    <p className="text-sm text-muted-foreground break-all">{lastOutputPath}</p>
                                </>
                            )}
                        </div>
                    )}
                </CardContent>
            </Card>
        </div>
    );
}
