import { useState, useEffect } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { FileText, Loader2, X } from 'lucide-react';
import { api } from '@/lib/api';
import type { Vendor, ValidationResult } from '@/types';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

interface CreateAuctionDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    onSuccess: () => void;
}

export function CreateAuctionDialog({ open, onOpenChange, onSuccess }: CreateAuctionDialogProps) {
    const [auctionName, setAuctionName] = useState('');
    const [csvFilePath, setCsvFilePath] = useState<string | null>(null);
    const [selectedVendorId, setSelectedVendorId] = useState<string | null>(null);
    const [vendors, setVendors] = useState<Vendor[]>([]);
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [isLoadingVendors, setIsLoadingVendors] = useState(false);
    const [validationResult, setValidationResult] = useState<ValidationResult | null>(null);
    const [isValidating, setIsValidating] = useState(false);
    const [editedCoefficients, setEditedCoefficients] = useState<Record<string, number>>({});

    useEffect(() => {
        if (open) {
            loadVendors();
            // Reset state
            setAuctionName('');
            setCsvFilePath(null);
            setSelectedVendorId(null);
            setIsSubmitting(false);
            setValidationResult(null);
            setIsValidating(false);
        }
    }, [open]);

    const loadVendors = async () => {
        try {
            setIsLoadingVendors(true);
            const data = await api.getVendors();
            const activeVendors = data.filter(v => v.is_active);
            setVendors(activeVendors);

            // Initialize edited coefficients
            const coefs: Record<string, number> = {};
            activeVendors.forEach(v => {
                coefs[v.id] = v.cost_coefficient;
            });
            setEditedCoefficients(coefs);
        } catch (err) {
            console.error('Failed to load vendors:', err);
        } finally {
            setIsLoadingVendors(false);
        }
    };

    const handleBrowseCsv = async () => {
        try {
            const filePath = await api.selectFile([{ name: 'CSV', extensions: ['csv'] }]);
            if (filePath && typeof filePath === 'string') {
                setCsvFilePath(filePath);

                setIsValidating(true);
                setValidationResult(null);
                try {
                    const result = await api.validateCsv(filePath);
                    setValidationResult(result);
                } catch (err: any) {
                    setValidationResult({ valid: false, message: err.toString(), warnings: [] });
                } finally {
                    setIsValidating(false);
                }
            }
        } catch (err) {
            console.error('Failed to select file:', err);
        }
    };

    const handleCreate = async () => {
        if (!auctionName.trim()) return;

        try {
            setIsSubmitting(true);

            // 0. Update any changed vendor coefficients
            for (const vendor of vendors) {
                const newCoef = editedCoefficients[vendor.id];
                if (newCoef !== undefined && newCoef !== vendor.cost_coefficient) {
                    try {
                        await api.updateVendor(vendor.id, {
                            cost_coefficient: newCoef,
                            min_price_margin: vendor.min_price_margin
                        });
                    } catch (e) {
                        console.error('Failed to update vendor coeff', e);
                    }
                }
            }

            // 1. Create Auction
            const auctionId = await api.createAuction({
                name: auctionName,
                vendor_id: selectedVendorId || undefined
            });

            // 2. Import Manifest if selected
            if (csvFilePath) {
                try {
                    await api.importManifest(csvFilePath, auctionId);
                } catch (err) {
                    console.error('Failed to import manifest:', err);
                    // Decide if we should block success. Usually, we still consider auction created.
                }
            }

            onSuccess();
            onOpenChange(false);
        } catch (err: any) {
            console.error('Failed to create auction:', err);
            alert(`Failed to create auction: ${err}`);
        } finally {
            setIsSubmitting(false);
        }
    };

    return (
        <Dialog.Root open={open} onOpenChange={onOpenChange}>
            <Dialog.Portal>
                <Dialog.Overlay className="fixed inset-0 bg-black/50 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 z-50" />
                <Dialog.Content className="fixed left-[50%] top-[50%] z-50 flex flex-col w-full max-w-lg max-h-[90vh] translate-x-[-50%] translate-y-[-50%] gap-4 border bg-background p-6 shadow-lg duration-200 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[state=closed]:slide-out-to-left-1/2 data-[state=closed]:slide-out-to-top-[48%] data-[state=open]:slide-in-from-left-1/2 data-[state=open]:slide-in-from-top-[48%] sm:rounded-lg">
                    <div className="flex flex-col space-y-1.5 text-center sm:text-left shrink-0">
                        <Dialog.Title className="text-lg font-semibold leading-none tracking-tight">
                            Create New Auction
                        </Dialog.Title>
                        <Dialog.Description className="sr-only">
                            Configure a new auction event. You can upload a manifest and assign a vendor pricing configuration.
                        </Dialog.Description>
                    </div>
                    <Dialog.Close className="absolute right-4 top-4 rounded-sm opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:pointer-events-none data-[state=open]:bg-accent data-[state=open]:text-muted-foreground">
                        <X className="h-4 w-4" />
                        <span className="sr-only">Close</span>
                    </Dialog.Close>

                    <div className="space-y-6 py-4 overflow-y-auto flex-1 px-1 pr-2">
                        {/* Section 1: Auction Name */}
                        <div className="space-y-2">
                            <label htmlFor="auction-name" className="text-sm font-medium leading-none">
                                Auction Name <span className="text-red-500">*</span>
                            </label>
                            <Input
                                id="auction-name"
                                placeholder="Name"
                                value={auctionName}
                                onChange={(e) => setAuctionName(e.target.value)}
                            />
                        </div>

                        {/* Section 2: CSV Upload */}
                        <div className="space-y-2">
                            <label className="text-sm font-medium leading-none">
                                Upload Manifest CSV
                            </label>
                            <div className="flex flex-col gap-2 mt-2">
                                <div className="flex items-center gap-2">
                                    <Button
                                        variant="outline"
                                        type="button"
                                        onClick={handleBrowseCsv}
                                        disabled={isValidating}
                                    >
                                        {isValidating ? <Loader2 className="h-4 w-4 animate-spin mr-2" /> : null}
                                        Browse
                                    </Button>
                                    <div className="flex-1 flex items-center gap-2 px-3 py-2 border rounded-md bg-muted/50 text-sm overflow-hidden text-ellipsis whitespace-nowrap">
                                        <FileText className="h-4 w-4 shrink-0 text-muted-foreground" />
                                        <span className="text-muted-foreground truncate">
                                            {csvFilePath ? csvFilePath.split('\\').pop()?.split('/').pop() : 'No file selected'}
                                        </span>
                                    </div>
                                </div>
                                {validationResult && (
                                    <div className={`text-sm p-3 rounded-md border ${validationResult.valid ? 'bg-green-500/10 border-green-500/20 text-green-700 dark:text-green-400' : 'bg-red-500/10 border-red-500/20 text-red-700 dark:text-red-400'}`}>
                                        <div className="font-medium">{validationResult.valid ? 'Manifest Valid' : 'Invalid Manifest'}</div>
                                        <div className="mt-1 opacity-90">{validationResult.message}</div>
                                        {validationResult.warnings && validationResult.warnings.length > 0 && (
                                            <ul className="mt-2 list-disc list-inside text-xs opacity-80">
                                                {validationResult.warnings.map((w, i) => <li key={i}>{w}</li>)}
                                            </ul>
                                        )}
                                    </div>
                                )}
                            </div>
                        </div>

                        {/* Section 3: Vendor Configuration */}
                        <div className="space-y-2">
                            <label className="text-sm font-medium leading-none">
                                Vendor Configuration
                            </label>
                            <p className="text-sm text-muted-foreground">
                                Select the vendor to apply pricing rules
                            </p>
                            {isLoadingVendors ? (
                                <div className="flex items-center gap-2 text-sm text-muted-foreground h-20 -mb-2">
                                    <Loader2 className="h-4 w-4 animate-spin" />
                                    Loading vendors...
                                </div>
                            ) : (
                                <div className="grid gap-2 mt-2 max-h-40 overflow-y-auto px-1 py-1">
                                    {vendors.map((vendor) => (
                                        <div
                                            key={vendor.id}
                                            onClick={() => setSelectedVendorId(vendor.id === selectedVendorId ? null : vendor.id)}
                                            className={`flex cursor-pointer items-center justify-between rounded-lg border p-3 transition-colors hover:bg-muted/50 ${selectedVendorId === vendor.id ? 'border-primary bg-primary/5 ring-1 ring-primary' : ''
                                                }`}
                                        >
                                            <div className="font-medium text-sm">{vendor.name}</div>
                                            <div className="text-xs text-muted-foreground flex items-center gap-1">
                                                Cost = Retail &times;
                                                <Input
                                                    type="number"
                                                    step="0.01"
                                                    min="0.01"
                                                    max="0.99"
                                                    className="w-16 h-6 px-1 py-0 text-xs inline-block"
                                                    value={editedCoefficients[vendor.id] !== undefined ? editedCoefficients[vendor.id] : vendor.cost_coefficient}
                                                    onChange={(e) => {
                                                        const val = parseFloat(e.target.value);
                                                        setEditedCoefficients(prev => ({
                                                            ...prev,
                                                            [vendor.id]: isNaN(val) ? 0 : val
                                                        }));
                                                    }}
                                                    onClick={(e) => e.stopPropagation()}
                                                />
                                            </div>
                                        </div>
                                    ))}
                                    {vendors.length === 0 && (
                                        <div className="text-sm text-muted-foreground italic py-2">
                                            No active vendors found.
                                        </div>
                                    )}
                                </div>
                            )}
                        </div>
                    </div>

                    <div className="flex flex-col-reverse sm:flex-row sm:justify-end sm:space-x-2 shrink-0 pt-2">
                        <Button
                            variant="outline"
                            onClick={() => onOpenChange(false)}
                            disabled={isSubmitting}
                        >
                            Cancel
                        </Button>
                        <Button
                            onClick={handleCreate}
                            disabled={!auctionName.trim() || isSubmitting}
                        >
                            {isSubmitting ? (
                                <>
                                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                                    Creating...
                                </>
                            ) : (
                                'Create Auction'
                            )}
                        </Button>
                    </div>
                </Dialog.Content>
            </Dialog.Portal>
        </Dialog.Root>
    );
}
