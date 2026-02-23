import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import { formatCurrency, formatDate, cn } from '@/lib/utils';
import type { InventoryItem } from '@/types';

interface Props {
    item: InventoryItem | null;
    open: boolean;
    onOpenChange: (open: boolean) => void;
}

export function InventoryItemDetailDialog({ item, open, onOpenChange }: Props) {
    if (!item) return null;

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="max-w-lg max-h-[85vh] overflow-y-auto">
                <DialogHeader>
                    <DialogTitle className="flex items-center gap-3">
                        <span className="font-mono text-muted-foreground text-sm">
                            Lot #{item.lot_number || '—'}
                        </span>
                        <Badge variant="secondary">{item.current_status}</Badge>
                    </DialogTitle>
                </DialogHeader>

                <p className="font-semibold text-base leading-tight">{item.raw_title}</p>

                <SectionBlock title="Financial">
                    <DataRow label="Retail Price" value={formatCurrency(item.retail_price)} />
                    <DataRow label="Cost Price" value={formatCurrency(item.cost_price)} />
                    <DataRow label="Min Bid" value={formatCurrency(item.min_price)} emphasized />
                </SectionBlock>

                <SectionBlock title="Product Info">
                    <DataRow label="Brand" value={item.extracted_brand} />
                    <DataRow label="Model" value={item.extracted_model} />
                    <DataRow label="SKU" value={item.sku_extracted} mono />
                    <DataRow label="Source" value={item.source} />
                    <DataRow label="Condition" value={item.condition} />
                    <DataRow label="Quantity" value={String(item.quantity)} />
                    <DataRow label="Category" value={item.category} />
                </SectionBlock>

                {item.auction_id && (
                    <SectionBlock title="Auction">
                        <DataRow label="Auction ID" value={item.auction_id} mono />
                        <DataRow label="Listed At" value={item.listed_at ? formatDate(item.listed_at) : null} />
                        <DataRow label="Sold At" value={item.sold_at ? formatDate(item.sold_at) : null} />
                    </SectionBlock>
                )}

                <SectionBlock title="System">
                    <DataRow label="Item ID" value={item.id} mono />
                    <DataRow label="Created" value={formatDate(item.created_at)} />
                </SectionBlock>
            </DialogContent>
        </Dialog>
    );
}

function SectionBlock({ title, children }: { title: string; children: React.ReactNode }) {
    return (
        <div className="space-y-2">
            <Separator />
            <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">{title}</p>
            <div className="space-y-1.5">{children}</div>
        </div>
    );
}

function DataRow({ label, value, mono, emphasized }: { label: string; value?: string | null; mono?: boolean; emphasized?: boolean }) {
    if (!value) return null; // Не рендерить пустые поля
    return (
        <div className="flex justify-between items-center text-sm">
            <span className="text-muted-foreground">{label}</span>
            <span className={cn(
                mono && 'font-mono text-xs',
                emphasized && 'font-semibold text-emerald-600'
            )}>
                {value}
            </span>
        </div>
    );
}
