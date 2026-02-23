import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';
import { Toaster } from 'sonner';

export function AppLayout() {
    return (
        <div className="flex h-screen overflow-hidden bg-background">
            <Sidebar />
            <main className="flex-1 overflow-y-auto">
                <div className="container mx-auto px-8 py-6">
                    <Outlet />
                </div>
            </main>
            <Toaster richColors position="top-right" />
        </div>
    );
}
