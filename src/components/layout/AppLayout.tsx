import { useEffect } from 'react';
import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';
import { Toaster, toast } from 'sonner';
import { check } from '@tauri-apps/plugin-updater';
import { ask } from '@tauri-apps/plugin-dialog';

export function AppLayout() {
    useEffect(() => {
        // Run update check on mount
        const checkForUpdates = async () => {
            try {
                const update = await check();
                if (update) {
                    // Ask user if they want to install
                    const yes = await ask(`Version ${update.version} is available! Do you want to download and install it now?\n\nRelease notes:\n${update.body}`, {
                        title: 'Update Available',
                        kind: 'info',
                    });

                    if (yes) {
                        toast.info('Downloading update...');
                        await update.downloadAndInstall();
                        // App will automatically restart after installation
                    }
                }
            } catch (err) {
                console.error('Failed to check for updates:', err);
                // Don't show toast for check failures to avoid annoying users if they are offline
            }
        };

        checkForUpdates();
    }, []);

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
