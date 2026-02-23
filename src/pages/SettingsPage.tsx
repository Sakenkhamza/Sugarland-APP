import { useEffect, useState } from 'react';
import { api } from '@/lib/api';
import type { Vendor } from '@/types';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import { toast } from 'sonner';

export function SettingsPage() {
    const [vendors, setVendors] = useState<Vendor[]>([]);
    const [vendorEdits, setVendorEdits] = useState<Record<string, Partial<Vendor>>>({});
    const [loading, setLoading] = useState(true);
    const [isSaving, setIsSaving] = useState(false);

    const [commissionRate, setCommissionRate] = useState<number>(15);
    const [commissionDirty, setCommissionDirty] = useState(false);

    useEffect(() => {
        loadVendors();
        loadSettings();
    }, []);

    const loadSettings = async () => {
        try {
            const rate = await api.getSettings('default_commission_rate');
            if (rate) setCommissionRate(parseFloat(rate) * 100);
        } catch (err) {
            console.error('Failed to load settings:', err);
        }
    };

    const loadVendors = async () => {
        try {
            setLoading(true);
            const data = await api.getVendors();
            setVendors(data);
            setVendorEdits({});
        } catch (err) {
            console.error('Failed to load vendors:', err);
            toast.error('Failed to load vendors');
        } finally {
            setLoading(false);
        }
    };

    const handleSaveVendors = async () => {
        setIsSaving(true);
        try {
            await Promise.all(
                Object.entries(vendorEdits).map(([vendorId, data]) =>
                    api.updateVendor(vendorId, data as { cost_coefficient: number; min_price_margin: number })
                )
            );
            toast.success('Vendor settings saved successfully');
            await loadVendors();
        } catch (err) {
            console.error('Failed to save vendor settings:', err);
            toast.error('Failed to save vendor settings');
        } finally {
            setIsSaving(false);
        }
    };

    return (
        <div className="space-y-8 max-w-3xl animate-fade-in">
            <div>
                <h1 className="text-3xl font-bold tracking-tight">Settings</h1>
                <p className="text-muted-foreground mt-1">
                    Configure vendors, pricing, and application preferences
                </p>
            </div>

            {/* Vendor Configuration */}
            <Card>
                <CardHeader>
                    <div className="flex items-center justify-between">
                        <div>
                            <CardTitle className="text-lg">Vendor Configuration</CardTitle>
                            <CardDescription>
                                Set pricing coefficients and minimum margins per vendor
                            </CardDescription>
                        </div>
                        <Button variant="outline" size="sm" onClick={loadVendors}>
                            Refresh
                        </Button>
                    </div>
                </CardHeader>
                <CardContent className="space-y-4">
                    {loading ? (
                        <div className="text-center py-8 text-muted-foreground">Loading vendors...</div>
                    ) : vendors.length === 0 ? (
                        <div className="text-center py-8 text-muted-foreground">No vendors configured.</div>
                    ) : (
                        vendors.map((vendor) => (
                            <div key={vendor.id} className="flex items-center gap-4 p-4 rounded-lg border hover:bg-muted/30 transition-colors">
                                <div className="flex-1">
                                    <div className="flex items-center gap-2">
                                        <p className="font-medium">{vendor.name}</p>
                                        <Badge variant={vendor.is_active ? 'default' : 'secondary'}>
                                            {vendor.id}
                                        </Badge>
                                    </div>
                                    <p className="text-xs text-muted-foreground mt-1">
                                        Cost = Retail × {vendor.cost_coefficient} ({vendor.cost_coefficient * 100}%)
                                    </p>
                                </div>
                                <div className="flex gap-3">
                                    <div className="text-center">
                                        <p className="text-xs text-muted-foreground mb-1">Coefficient</p>
                                        <Input
                                            className="w-24 text-center font-mono"
                                            value={vendorEdits[vendor.id]?.cost_coefficient ?? vendor.cost_coefficient}
                                            onChange={(e) => setVendorEdits(prev => ({
                                                ...prev,
                                                [vendor.id]: {
                                                    ...(prev[vendor.id] || vendor),
                                                    cost_coefficient: parseFloat(e.target.value) || 0
                                                }
                                            }))}
                                            type="number"
                                            step="0.01"
                                        />
                                    </div>
                                    <div className="text-center">
                                        <p className="text-xs text-muted-foreground mb-1">Min Margin</p>
                                        <Input
                                            className="w-24 text-center font-mono"
                                            value={vendorEdits[vendor.id]?.min_price_margin ?? vendor.min_price_margin}
                                            onChange={(e) => setVendorEdits(prev => ({
                                                ...prev,
                                                [vendor.id]: {
                                                    ...(prev[vendor.id] || vendor),
                                                    min_price_margin: parseFloat(e.target.value) || 0
                                                }
                                            }))}
                                            type="number"
                                            step="0.01"
                                        />
                                    </div>
                                </div>
                            </div>
                        ))
                    )}
                    <Button
                        className="mt-4 w-full sm:w-auto"
                        onClick={handleSaveVendors}
                        disabled={Object.keys(vendorEdits).length === 0 || isSaving}
                    >
                        {isSaving ? 'Saving...' : 'Save Changes'}
                    </Button>
                </CardContent>
            </Card>

            <Separator />

            {/* Buyback Detection */}
            <Card>
                <CardHeader>
                    <CardTitle className="text-lg">Buyback Detection</CardTitle>
                    <CardDescription>Configure automatic buyback identification</CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div className="flex items-center gap-4 p-4 rounded-lg border">
                        <div className="flex-1">
                            <p className="font-medium text-sm">Ron Larsson Detection</p>
                            <p className="text-xs text-muted-foreground">
                                Bidder ID 5046 automatically flagged as buyback
                            </p>
                        </div>
                        <Badge className="bg-emerald-600 hover:bg-emerald-700">Active</Badge>
                    </div>
                    <div className="space-y-2">
                        <label className="text-sm font-medium">Default Commission Rate (%)</label>
                        <Input
                            className="w-32"
                            value={commissionRate}
                            type="number"
                            step="0.1"
                            min="0"
                            max="100"
                            onChange={(e) => {
                                setCommissionRate(parseFloat(e.target.value));
                                setCommissionDirty(true);
                            }}
                        />
                        <p className="text-xs text-muted-foreground">
                            Applied to all non-buyback sales
                        </p>
                        <Button
                            className="mt-2"
                            disabled={!commissionDirty || isSaving}
                            onClick={async () => {
                                setIsSaving(true);
                                try {
                                    await api.saveSetting('default_commission_rate', String(commissionRate / 100));
                                    setCommissionDirty(false);
                                    toast.success('Commission rate saved');
                                } catch (err) {
                                    console.error('Failed to save rate:', err);
                                    toast.error('Failed to save commission rate');
                                } finally {
                                    setIsSaving(false);
                                }
                            }}
                        >
                            Save Rate
                        </Button>
                    </div>
                </CardContent>
            </Card>

            <Separator />

            {/* Database */}
            <Card>
                <CardHeader>
                    <CardTitle className="text-lg">Database</CardTitle>
                    <CardDescription>SQLite database management</CardDescription>
                </CardHeader>
                <CardContent className="space-y-3">
                    <div className="flex items-center justify-between p-3 rounded-lg bg-muted/50 border">
                        <div>
                            <p className="font-medium text-sm">Database Path</p>
                            <p className="text-xs text-muted-foreground font-mono">sugarland.db</p>
                        </div>
                        <Badge variant="outline" className="border-emerald-500 text-emerald-600">
                            Connected
                        </Badge>
                    </div>
                    <div className="flex gap-3">
                        <Button variant="outline" size="sm">Export Database</Button>
                        <Button variant="outline" size="sm">Run Migrations</Button>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
