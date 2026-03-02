import { useState, useEffect, ReactNode } from 'react';
import { check } from '@tauri-apps/plugin-updater';
import { ask, message } from '@tauri-apps/plugin-dialog';
import { Loader2 } from 'lucide-react';

interface UpdateGuardProps {
    children: ReactNode;
}

export function UpdateGuard({ children }: UpdateGuardProps) {
    const [isReady, setIsReady] = useState(false);
    const [statusText, setStatusText] = useState('Проверка обновлений...');
    const [progress, setProgress] = useState<number | null>(null);

    useEffect(() => {
        let isMounted = true;

        const checkUpdate = async () => {
            try {
                const update = await check();

                if (update) {
                    if (isMounted) setStatusText(`Найдена версия ${update.version}`);

                    const yes = await ask(`Доступна новая версия ${update.version}!\n\nЧто нового:\n${update.body}\n\nХотите скачать и установить её сейчас?`, {
                        title: 'Доступно обновление',
                        kind: 'info',
                        cancelLabel: 'Позже',
                        okLabel: 'Обновить'
                    });

                    if (yes) {
                        if (isMounted) setStatusText('Скачивание обновления...');
                        let downloaded = 0;
                        let contentLength = 0;

                        await update.downloadAndInstall((event) => {
                            switch (event.event) {
                                case 'Started':
                                    contentLength = event.data.contentLength || 0;
                                    break;
                                case 'Progress':
                                    downloaded += event.data.chunkLength;
                                    if (contentLength > 0 && isMounted) {
                                        setProgress(Math.round((downloaded / contentLength) * 100));
                                    }
                                    break;
                                case 'Finished':
                                    if (isMounted) setStatusText('Установка обновления...');
                                    break;
                            }
                        });
                        // App will automatically restart after installation
                    } else {
                        // User declined update
                        if (isMounted) setIsReady(true);
                    }
                } else {
                    // No updates available
                    if (isMounted) setIsReady(true);
                }
            } catch (err: unknown) {
                console.error('[Updater] Failed to check for updates:', err);
                await message(`Ошибка при проверке обновлений: ${err instanceof Error ? err.message : String(err)}`, {
                    title: 'Ошибка',
                    kind: 'error'
                });
                if (isMounted) setIsReady(true);
            }
        };

        checkUpdate();

        return () => {
            isMounted = false;
        };
    }, []);

    if (!isReady) {
        return (
            <div className="flex flex-col items-center justify-center h-screen w-screen bg-background text-foreground">
                <Loader2 className="h-12 w-12 animate-spin text-primary mb-6" />
                <h2 className="text-2xl font-semibold mb-2">{statusText}</h2>
                {progress !== null && (
                    <div className="w-64 mt-4">
                        <div className="flex justify-between text-sm mb-1 text-muted-foreground">
                            <span>Загрузка...</span>
                            <span>{progress}%</span>
                        </div>
                        <div className="w-full bg-secondary rounded-full h-2">
                            <div
                                className="bg-primary h-2 rounded-full transition-all duration-300"
                                style={{ width: `${progress}%` }}
                            ></div>
                        </div>
                    </div>
                )}
            </div>
        );
    }

    return <>{children}</>;
}
