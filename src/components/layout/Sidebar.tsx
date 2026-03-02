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
        <header className="flex h-16 w-full items-center justify-between border-b bg-card/50 backdrop-blur-sm px-6 shrink-0 z-10">
            {/* Logo */}
            <div className="flex items-center gap-3">
                <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary shadow-lg shadow-primary/25">
                    <Candy className="h-5 w-5 text-primary-foreground" />
                </div>
                <div className="flex flex-col">
                    <h1 className="text-base font-bold tracking-tight leading-tight">Sugarland</h1>
                    <p className="text-[10px] text-muted-foreground font-medium leading-tight">Liquidation</p>
                </div>
            </div>

            {/* Navigation */}
            <nav className="flex items-center gap-1 flex-1 justify-center px-4 overflow-x-auto no-scrollbar">
                {navigation.map((item) => (
                    <NavLink
                        key={item.name}
                        to={item.href}
                        className={({ isActive }) =>
                            cn(
                                'flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-all duration-200 whitespace-nowrap',
                                isActive
                                    ? 'bg-primary/10 text-primary shadow-sm'
                                    : 'text-muted-foreground hover:bg-accent hover:text-foreground'
                            )
                        }
                    >
                        <item.icon className="h-4 w-4" />
                        {item.name}
                    </NavLink>
                ))}
            </nav>

            {/* Footer / Info */}
            <div className="flex flex-col items-end">
                <p className="text-xs font-medium text-foreground">v{appVersion}</p>
                <p className="text-[10px] text-muted-foreground">Phase 1</p>
            </div>
        </header>
    );
}
