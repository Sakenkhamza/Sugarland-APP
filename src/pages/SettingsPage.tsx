import { useEffect, useState } from 'react';
import { api } from '@/lib/api';
import type { Buybacker } from '@/types';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import { toast } from 'sonner';

export function SettingsPage() {
    const [isWiping, setIsWiping] = useState(false);



    // Buy-backers state
    const [buybackers, setBuybackers] = useState<Buybacker[]>([]);
    const [buybackersLoading, setBuybackersLoading] = useState(true);
    const [editingBuybackerId, setEditingBuybackerId] = useState<string | null>(null);
    const [editName, setEditName] = useState('');
    const [newBuybackerName, setNewBuybackerName] = useState('');
    const [deletingId, setDeletingId] = useState<string | null>(null);

    useEffect(() => {
        loadBuybackers();
    }, []);



    const loadBuybackers = async () => {
        try {
            setBuybackersLoading(true);
            const data = await api.getBuybackers();
            setBuybackers(data);
        } catch (err) {
            console.error('Failed to load buybackers:', err);
            toast.error('Failed to load buy-backers');
        } finally {
            setBuybackersLoading(false);
        }
    };

    const handleAddBuybacker = async () => {
        const name = newBuybackerName.trim();
        if (!name) {
            toast.error('Name is required');
            return;
        }
        try {
            await api.addBuybacker(name);
            toast.success(`Buy-backer "${name}" added`);
            setNewBuybackerName('');
            await loadBuybackers();
        } catch (err) {
            console.error('Failed to add buybacker:', err);
            toast.error('Failed to add buy-backer');
        }
    };

    const startEdit = (bb: Buybacker) => {
        setEditingBuybackerId(bb.id);
        setEditName(bb.name);
    };

    const cancelEdit = () => {
        setEditingBuybackerId(null);
        setEditName('');
    };

    const handleSaveBuybacker = async () => {
        if (!editingBuybackerId) return;
        const name = editName.trim();
        if (!name) {
            toast.error('Name is required');
            return;
        }
        try {
            await api.updateBuybacker(editingBuybackerId, name);
            toast.success('Buy-backer updated');
            cancelEdit();
            await loadBuybackers();
        } catch (err) {
            console.error('Failed to update buybacker:', err);
            toast.error('Failed to update buy-backer');
        }
    };

    const handleDeleteBuybacker = async (id: string) => {
        try {
            await api.deleteBuybacker(id);
            toast.success('Buy-backer deleted');
            setDeletingId(null);
            await loadBuybackers();
        } catch (err) {
            console.error('Failed to delete buybacker:', err);
            toast.error('Failed to delete buy-backer');
        }
    };


    const handleWipeDatabase = async () => {
        if (!window.confirm("WARNING: This will permanently delete ALL inventory, auctions, and historical sales data. Are you absolutely sure?")) {
            return;
        }
        
        // Double confirmation for extra safety
        if (!window.confirm("FINAL WARNING: This action CANNOT be undone. Proceed with wiping the database?")) {
            return;
        }

        setIsWiping(true);
        try {
            await api.wipeDatabase();
            toast.success("Database wiped successfully. All operational data has been deleted.");
            // Reload components relying on the DB
            await loadBuybackers();
        } catch (err) {
            console.error("Failed to wipe database:", err);
            toast.error("Failed to wipe database");
        } finally {
            setIsWiping(false);
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

            {/* Buy-backers Management */}
            <Card>
                <CardHeader>
                    <div className="flex items-center justify-between">
                        <div>
                            <CardTitle className="text-lg">Buy-backer Registry</CardTitle>
                            <CardDescription>
                                Manage buy-backers who are automatically flagged as buyback during reconciliation based on their name.
                            </CardDescription>
                        </div>
                        <Button variant="outline" size="sm" onClick={loadBuybackers}>
                            Refresh
                        </Button>
                    </div>
                </CardHeader>
                <CardContent className="space-y-4">
                    {buybackersLoading ? (
                        <div className="text-center py-8 text-muted-foreground">Loading buy-backers...</div>
                    ) : buybackers.length === 0 ? (
                        <div className="text-center py-6 text-muted-foreground">No buy-backers configured yet.</div>
                    ) : (
                        <div className="space-y-2">
                            {buybackers.map((bb) => (
                                <div key={bb.id} className="flex items-center gap-3 p-3 rounded-lg border hover:bg-muted/30 transition-colors">
                                    {editingBuybackerId === bb.id ? (
                                        /* ---- Editing mode ---- */
                                        <>
                                            <div className="flex-1 flex gap-2">
                                                <Input
                                                    className="flex-1"
                                                    value={editName}
                                                    onChange={(e) => setEditName(e.target.value)}
                                                    placeholder="Name"
                                                />
                                            </div>
                                            <Button size="sm" onClick={handleSaveBuybacker}>
                                                Save
                                            </Button>
                                            <Button size="sm" variant="ghost" onClick={cancelEdit}>
                                                Cancel
                                            </Button>
                                        </>
                                    ) : deletingId === bb.id ? (
                                        /* ---- Delete confirmation ---- */
                                        <>
                                            <div className="flex-1">
                                                <p className="text-sm text-destructive font-medium">
                                                    Delete "{bb.name}"?
                                                </p>
                                            </div>
                                            <Button size="sm" variant="destructive" onClick={() => handleDeleteBuybacker(bb.id)}>
                                                Confirm
                                            </Button>
                                            <Button size="sm" variant="ghost" onClick={() => setDeletingId(null)}>
                                                Cancel
                                            </Button>
                                        </>
                                    ) : (
                                        /* ---- Display mode ---- */
                                        <>
                                            <div className="flex-1">
                                                <div className="flex items-center gap-2">
                                                    <p className="font-medium text-sm">{bb.name}</p>
                                                </div>
                                            </div>
                                            <Button size="sm" variant="ghost" onClick={() => startEdit(bb)} title="Edit">
                                                ✏️
                                            </Button>
                                            <Button size="sm" variant="ghost" onClick={() => setDeletingId(bb.id)} title="Delete">
                                                🗑️
                                            </Button>
                                        </>
                                    )}
                                </div>
                            ))}
                        </div>
                    )}

                    {/* Add new buy-backer row */}
                    <div className="flex items-center gap-2 p-3 rounded-lg border border-dashed">
                        <Input
                            className="flex-1"
                            value={newBuybackerName}
                            onChange={(e) => setNewBuybackerName(e.target.value)}
                            placeholder="New buy-backer name"
                        />
                        <Button size="sm" onClick={handleAddBuybacker} disabled={!newBuybackerName.trim()}>
                            + Add
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
                    <div className="flex gap-3 mt-4">
                        <Button variant="outline" size="sm">Export Database</Button>
                        <Button variant="outline" size="sm">Run Migrations</Button>
                    </div>

                    <Separator className="my-4" />
                    
                    <div>
                        <h4 className="text-sm font-medium text-destructive mb-2">Danger Zone</h4>
                        <p className="text-sm text-muted-foreground mb-4">
                            Permanently delete all operational data (inventory, manifests, auctions, history). This action cannot be undone.
                        </p>
                        <Button 
                            variant="destructive" 
                            onClick={handleWipeDatabase}
                            disabled={isWiping}
                        >
                            {isWiping ? 'Wiping...' : 'Wipe Database'}
                        </Button>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
