import { ReactNode, useEffect, useState } from 'react';
import { ask, message } from '@tauri-apps/plugin-dialog';
import { check } from '@tauri-apps/plugin-updater';
import { Loader2 } from 'lucide-react';

interface UpdateGuardProps {
    children: ReactNode;
}

const UPDATE_CHECK_MAX_ATTEMPTS = 2;
const UPDATE_CHECK_RETRY_DELAY_MS = 3000;

function delay(ms: number) {
    return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function getErrorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
}

async function checkForUpdatesWithRetry() {
    let lastError: unknown = null;

    for (let attempt = 1; attempt <= UPDATE_CHECK_MAX_ATTEMPTS; attempt += 1) {
        try {
            return await check();
        } catch (err) {
            lastError = err;
            if (attempt < UPDATE_CHECK_MAX_ATTEMPTS) {
                await delay(UPDATE_CHECK_RETRY_DELAY_MS);
            }
        }
    }

    throw lastError;
}

export function UpdateGuard({ children }: UpdateGuardProps) {
    const [isReady, setIsReady] = useState(false);
    const [statusText, setStatusText] = useState('Проверка обновлений...');
    const [progress, setProgress] = useState<number | null>(null);

    useEffect(() => {
        let isMounted = true;

        const checkUpdate = async () => {
            let update: Awaited<ReturnType<typeof check>> | null = null;

            try {
                update = await checkForUpdatesWithRetry();
            } catch (err) {
                console.warn('[Updater] Skipping update check after failure:', err);
                if (isMounted) {
                    setStatusText('Не удалось проверить обновления. Запускаем приложение...');
                    setIsReady(true);
                }

                if (import.meta.env.DEV) {
                    await message(`Ошибка при проверке обновлений: ${getErrorMessage(err)}`, {
                        title: 'Ошибка',
                        kind: 'error',
                    });
                }
                return;
            }

            if (!update) {
                if (isMounted) setIsReady(true);
                return;
            }

            if (isMounted) setStatusText(`Найдена версия ${update.version}`);

            const yes = await ask(
                `Доступна новая версия ${update.version}!\n\nЧто нового:\n${update.body}\n\nХотите скачать и установить её сейчас?`,
                {
                    title: 'Доступно обновление',
                    kind: 'info',
                    cancelLabel: 'Позже',
                    okLabel: 'Обновить',
                },
            );

            if (!yes) {
                if (isMounted) setIsReady(true);
                return;
            }

            try {
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
                // The app restarts automatically after a successful install.
            } catch (err) {
                console.error('[Updater] Failed to download/install update:', err);
                await message(`Не удалось установить обновление: ${getErrorMessage(err)}`, {
                    title: 'Ошибка',
                    kind: 'error',
                });
                if (isMounted) {
                    setProgress(null);
                    setIsReady(true);
                }
            }
        };

        void checkUpdate();

        return () => {
            isMounted = false;
        };
    }, []);

    if (!isReady) {
        return (
            <div className="flex h-screen w-screen flex-col items-center justify-center bg-background text-foreground">
                <Loader2 className="mb-6 h-12 w-12 animate-spin text-primary" />
                <h2 className="mb-2 text-2xl font-semibold">{statusText}</h2>
                {progress !== null && (
                    <div className="mt-4 w-64">
                        <div className="mb-1 flex justify-between text-sm text-muted-foreground">
                            <span>Загрузка...</span>
                            <span>{progress}%</span>
                        </div>
                        <div className="h-2 w-full rounded-full bg-secondary">
                            <div
                                className="h-2 rounded-full bg-primary transition-all duration-300"
                                style={{ width: `${progress}%` }}
                            />
                        </div>
                    </div>
                )}
            </div>
        );
    }

    return <>{children}</>;
}
