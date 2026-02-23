import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { AppLayout } from '@/components/layout/AppLayout';
import { DashboardPage } from '@/pages/DashboardPage';
import { ImportPage } from '@/pages/ImportPage';
import { InventoryPage } from '@/pages/InventoryPage';
import { AuctionsPage } from '@/pages/AuctionsPage';
import { ReconciliationPage } from '@/pages/ReconciliationPage';
import { ReportsPage } from '@/pages/ReportsPage';
import { SettingsPage } from '@/pages/SettingsPage';
import { AuctionDetailPage } from '@/pages/AuctionDetailPage';
import './index.css';

createRoot(document.getElementById('root')!).render(
    <StrictMode>
        <BrowserRouter>
            <Routes>
                <Route element={<AppLayout />}>
                    <Route path="/" element={<DashboardPage />} />
                    <Route path="/import" element={<ImportPage />} />
                    <Route path="/inventory" element={<InventoryPage />} />
                    <Route path="/auctions" element={<AuctionsPage />} />
                    <Route path="/auctions/:id" element={<AuctionDetailPage />} />
                    <Route path="/reconciliation" element={<ReconciliationPage />} />
                    <Route path="/reports" element={<ReportsPage />} />
                    <Route path="/settings" element={<SettingsPage />} />
                </Route>
            </Routes>
        </BrowserRouter>
    </StrictMode>
);
