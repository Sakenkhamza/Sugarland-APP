import { NavLink } from 'react-router-dom';
import { useEffect, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import {
    LayoutDashboard,
    Upload,
    Package,
    Gavel,
    FileBarChart,
    Settings,
    ArrowLeftRight,
    Candy,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import { Separator } from '@/components/ui/separator';

const navigation = [
    { name: 'Dashboard', href: '/', icon: LayoutDashboard },
    { name: 'Import', href: '/import', icon: Upload },
    { name: 'Inventory', href: '/inventory', icon: Package },
    { name: 'Auctions', href: '/auctions', icon: Gavel },
    { name: 'Reconciliation', href: '/reconciliation', icon: ArrowLeftRight },
    { name: 'Reports', href: '/reports', icon: FileBarChart },
    { name: 'Settings', href: '/settings', icon: Settings },
];

export function Sidebar() {
    const [appVersion, setAppVersion] = useState<string>('0.1.0');

    useEffect(() => {
        getVersion().then((version) => setAppVersion(version)).catch(console.error);
    }, []);

    return (
        <aside className="flex h-screen w-64 flex-col border-r bg-card/50 backdrop-blur-sm">
            {/* Logo */}
            <div className="flex items-center gap-3 px-6 py-5">
                <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary shadow-lg shadow-primary/25">
                    <Candy className="h-5 w-5 text-primary-foreground" />
                </div>
                <div>
                    <h1 className="text-lg font-bold tracking-tight">Sugarland</h1>
                    <p className="text-[11px] text-muted-foreground font-medium">Liquidation Platform</p>
                </div>
            </div>

            <Separator />

            {/* Navigation */}
            <nav className="flex-1 space-y-1 px-3 py-4">
                {navigation.map((item) => (
                    <NavLink
                        key={item.name}
                        to={item.href}
                        className={({ isActive }) =>
                            cn(
                                'flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-all duration-200',
                                isActive
                                    ? 'bg-primary/10 text-primary shadow-sm'
                                    : 'text-muted-foreground hover:bg-accent hover:text-foreground'
                            )
                        }
                    >
                        <item.icon className="h-4.5 w-4.5" />
                        {item.name}
                    </NavLink>
                ))}
            </nav>

            <Separator />

            {/* Footer */}
            <div className="px-6 py-4">
                <p className="text-xs text-muted-foreground">v{appVersion} — Phase 1 MVP</p>
            </div>
        </aside>
    );
}
