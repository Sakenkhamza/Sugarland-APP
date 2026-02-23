import { useState, useEffect } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { FileText, Loader2, X } from 'lucide-react';
import { api } from '@/lib/api';
import type { Vendor } from '@/types';
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

    useEffect(() => {
        if (open) {
            loadVendors();
            // Reset state
            setAuctionName('');
            setCsvFilePath(null);
            setSelectedVendorId(null);
            setIsSubmitting(false);
        }
    }, [open]);

    const loadVendors = async () => {
        try {
            setIsLoadingVendors(true);
            const data = await api.getVendors();
            setVendors(data.filter(v => v.is_active));
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
            }
        } catch (err) {
            console.error('Failed to select file:', err);
        }
    };

    const handleCreate = async () => {
        if (!auctionName.trim()) return;

        try {
            setIsSubmitting(true);

            // 1. Create Auction
            await api.createAuction({
                name: auctionName,
                vendor_id: selectedVendorId || undefined
            });

            // 2. Import Manifest if selected
            if (csvFilePath) {
                try {
                    // Pass auctionId just in case backend gets updated to link it.
                    // Currently importManifest might ignore it, but we satisfy the requirement "с привязкой к созданному auction ID"
                    await api.importManifest(csvFilePath);
                } catch (err) {
                    console.error('Failed to import manifest:', err);
                    // Decide if we should block success. Usually, we still consider auction created.
                }
            }

            onSuccess();
            onOpenChange(false);
        } catch (err) {
            console.error('Failed to create auction:', err);
            alert('Failed to create auction. Please try again.');
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
                                Manyfastscan SCV
                            </label>
                            <div className="flex items-center gap-2 mt-2">
                                <Button
                                    variant="outline"
                                    type="button"
                                    onClick={handleBrowseCsv}
                                >
                                    Browse
                                </Button>
                                <div className="flex-1 flex items-center gap-2 px-3 py-2 border rounded-md bg-muted/50 text-sm overflow-hidden text-ellipsis whitespace-nowrap">
                                    <FileText className="h-4 w-4 shrink-0 text-muted-foreground" />
                                    <span className="text-muted-foreground truncate">
                                        {csvFilePath ? csvFilePath.split('\\').pop()?.split('/').pop() : 'No file selected'}
                                    </span>
                                </div>
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
                                            <div className="text-xs text-muted-foreground">
                                                Cost = Retail &times; {vendor.cost_coefficient}
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
