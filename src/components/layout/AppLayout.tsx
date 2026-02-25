import { useEffect } from 'react';
import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';
import { Toaster, toast } from 'sonner';
import { check } from '@tauri-apps/plugin-updater';
import { ask } from '@tauri-apps/plugin-dialog';

export function AppLayout() {
    useEffect(() => {
        // Run update check on mount (with a small delay to let the app fully initialize)
        const timer = setTimeout(async () => {
            try {
                console.log('[Updater] Checking for updates...');
                const update = await check();
                console.log('[Updater] Check result:', update);

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
                } else {
                    console.log('[Updater] No updates available.');
                }
            } catch (err: unknown) {
                console.error('[Updater] Failed to check for updates:', err);
                toast.error(`Update check failed: ${err instanceof Error ? err.message : String(err)}`);
            }
        }, 3000);

        return () => clearTimeout(timer);
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
