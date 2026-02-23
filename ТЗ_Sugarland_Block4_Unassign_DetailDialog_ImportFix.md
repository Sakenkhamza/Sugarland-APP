# Техническое задание: Блок #4 — Закрытие хвостов
**Проект:** Sugarland — Liquidation Management  
**Стек:** React + TypeScript + Tauri (Rust) + Tailwind + Radix UI  
**Блок:** #4 — Финальная доводка UI и функциональных дыр  

---

## Что проанализировано в актуальном коде

После трёх блоков app технически замкнут, но при реальном использовании немедленно обнаружится 6 проблем:

| # | Место | Проблема |
|---|---|---|
| 1 | `AuctionDetailPage` | Кнопка `×` (unassign) выдаёт `toast.info("...not supported by API yet")` — сломана |
| 2 | `AuctionDetailPage` / `update_item_status` | При возврате лота в InStock (`update_item_status`) счётчик `total_lots` в таблице `auctions` **не уменьшается** — счётчик на Dashboard и в AuctionsPage врёт |
| 3 | `AuctionsPage` | Кнопка `CSV` в строке таблицы вызывает `api.selectFile()` (диалог **открытия**) и показывает `alert()` вместо toast — регрессия |
| 4 | `AuctionsPage` | Пустой `<div className="grid gap-4 md:grid-cols-3">` рендерит лишнее пустое пространство |
| 5 | `InventoryPage` | `View Details` — disabled без какого-либо функционала |
| 6 | `ImportPage` | После успешного импорта кнопка "View Inventory" закомментирована. CSV-валидация (`validate_csv` команда зарегистрирована в Rust) нигде не вызывается на фронте |

Дополнительно: `<select>` на ReconciliationPage — нативный HTML-элемент, стилизованный inline, а не Radix `Select`. Мелочь, но стоит привести к единому стилю.

---

## Изменение #1 — Unassign item: починить кнопку × и счётчик total_lots

### 1.1 Новая Rust-команда `unassign_item`

Добавить в `src-tauri/src/auctions.rs`:

```rust
#[tauri::command]
pub fn unassign_item(
    item_id: String,
    state: State<crate::AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    
    // Получить auction_id до сброса
    let auction_id: Option<String> = db.conn.query_row(
        "SELECT auction_id FROM inventory_items WHERE id = ?1",
        rusqlite::params![item_id],
        |r| r.get(0),
    ).unwrap_or(None);

    // Сбросить статус и auction_id
    db.conn.execute(
        "UPDATE inventory_items SET current_status = 'InStock', auction_id = NULL, listed_at = NULL WHERE id = ?1",
        rusqlite::params![item_id],
    ).map_err(|e| e.to_string())?;

    // Обновить счётчик total_lots в аукционе
    if let Some(auc_id) = auction_id {
        db.conn.execute(
            "UPDATE auctions SET total_lots = (
                SELECT COUNT(*) FROM inventory_items WHERE auction_id = ?1
             ) WHERE id = ?1",
            rusqlite::params![auc_id],
        ).map_err(|e| e.to_string())?;
    }

    Ok(())
}
```

Зарегистрировать в `main.rs` в `invoke_handler`:
```rust
auctions::unassign_item,
```

### 1.2 Исправить `update_item_status` в `main.rs`

Текущий `update_item_status` не обновляет `total_lots`. Добавить после UPDATE:

```rust
// Если статус сбрасывается в InStock — обновить счётчик аукциона
if status == "InStock" {
    // auction_id уже NULL после UPDATE, нужно получить его ДО update
    // Поэтому лучше использовать отдельную команду unassign_item
}
```

Фактически: **рекомендуется использовать `unassign_item` вместо `update_item_status` для случая снятия с аукциона**. `update_item_status` оставить только для `Scrap`.

### 1.3 Новый API-метод

В `src/lib/api.ts`:
```ts
unassignItem: (itemId: string) =>
    invokeCommand<void>('unassign_item', { itemId }),
```

**Mock:**
```ts
case 'unassign_item':
    return null;
```

### 1.4 Подключить в `AuctionDetailPage.tsx`

Заменить текущий `onClick` кнопки `×`:

```tsx
// Было:
onClick={() => toast.info("Unassign item implicitly not supported by API yet")}

// Стало:
onClick={async () => {
    try {
        await api.unassignItem(item.id);
        toast.success('Item removed from auction');
        await loadAuctionData(auction.id);
    } catch (err) {
        toast.error('Failed to remove item');
    }
}}
```

---

## Изменение #2 — AuctionsPage: починить CSV export и убрать пустой grid

### 2.1 Исправить `handleExport`

Текущий код (строки ~2459–2473):
```tsx
const handleExport = async (auction: Auction) => {
    const filePath = await api.selectFile(); // ← OPEN dialog, не SAVE
    if (!filePath) return;
    await api.exportAuctionCsv(auction.id, filePath as string);
    alert('Export successful!'); // ← alert(), не toast
};
```

Заменить на:
```tsx
const handleExport = async (auction: Auction) => {
    try {
        const defaultName = `${auction.name.replace(/\s+/g, '_')}_lots.csv`;
        const savePath = await api.saveFile(defaultName);
        if (!savePath) return;
        await api.exportAuctionCsv(auction.id, savePath as string);
        toast.success(`Exported ${auction.name} to CSV`);
    } catch (err) {
        toast.error('Export failed');
    }
};
```

Добавить импорт `toast` из `sonner`, если ещё нет.

### 2.2 Заменить пустой grid на KPI-карточки

Вместо:
```tsx
<div className="grid gap-4 md:grid-cols-3">
    {/* Quick Stats or Active Auctions could go here */}
</div>
```

Показать три счётчика по статусам аукционов (вычисляется из уже загруженного `auctions`):
```tsx
const draftCount = auctions.filter(a => a.status === 'Draft').length;
const activeCount = auctions.filter(a => a.status === 'Active').length;
const completedCount = auctions.filter(a => a.status === 'Completed').length;

<div className="grid gap-4 md:grid-cols-3">
    <Card>
        <CardContent className="pt-6">
            <div className="text-2xl font-bold">{draftCount}</div>
            <p className="text-sm text-muted-foreground">Draft</p>
        </CardContent>
    </Card>
    <Card>
        <CardContent className="pt-6">
            <div className="text-2xl font-bold text-blue-600">{activeCount}</div>
            <p className="text-sm text-muted-foreground">Active</p>
        </CardContent>
    </Card>
    <Card>
        <CardContent className="pt-6">
            <div className="text-2xl font-bold text-emerald-600">{completedCount}</div>
            <p className="text-sm text-muted-foreground">Completed</p>
        </CardContent>
    </Card>
</div>
```

Карточки рендерятся мгновенно из уже загруженных данных — дополнительных запросов не нужно.

---

## Изменение #3 — InventoryItemDetailDialog

### 3.1 Создать новый файл `src/components/inventory/InventoryItemDetailDialog.tsx`

Sheet (боковая панель) из `@radix-ui/react-dialog` или использовать стандартный `Dialog`. Показывает полную информацию по лоту.

**Props:**
```tsx
interface Props {
    item: InventoryItem | null;
    open: boolean;
    onOpenChange: (open: boolean) => void;
}
```

**Содержимое панели:**

```
┌─────────────────────────────────────┐
│  Lot #42m                    [×]    │
│                                     │
│  Samsung 65" Class 4K UHD Smart TV  │
│  [Badge: Listed]  [Badge: Electronics] │
│                                     │
│  ── Financial ──────────────────── │
│  Retail Price:   $549.99            │
│  Cost Price:     $77.00             │
│  Min Bid Price:  $132.00            │
│                                     │
│  ── Product Info ───────────────── │
│  Brand:          Samsung            │
│  Model:          UN65TU7000         │
│  SKU:            UN65TU7000FXZA     │
│  Source:         Best Buy           │
│  Condition:      New                │
│  Quantity:       1                  │
│                                     │
│  ── Auction Info ───────────────── │
│  Auction ID:     [auction_id]       │
│  Listed At:      Feb 15, 2026       │
│  Sold At:        —                  │
│                                     │
│  ── System ─────────────────────── │
│  Item ID:        abc123             │
│  Manifest ID:    xyz789             │
│  Created:        Feb 10, 2026       │
│  Updated:        Feb 15, 2026       │
└─────────────────────────────────────┘
```

Все поля берутся из `item` — никаких дополнительных API-запросов.

**Структура компонента:**
```tsx
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import { formatCurrency, formatDate } from '@/lib/utils';
import type { InventoryItem } from '@/types';

export function InventoryItemDetailDialog({ item, open, onOpenChange }: Props) {
    if (!item) return null;

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="max-w-lg max-h-[85vh] overflow-y-auto">
                <DialogHeader>
                    <DialogTitle className="flex items-center gap-3">
                        <span className="font-mono text-muted-foreground text-sm">
                            Lot #{item.lot_number || '—'}
                        </span>
                        <Badge ...>{item.current_status}</Badge>
                    </DialogTitle>
                </DialogHeader>
                
                <p className="font-semibold text-base leading-tight">{item.raw_title}</p>
                
                {/* Секции с данными */}
                <SectionBlock title="Financial">
                    <DataRow label="Retail Price" value={formatCurrency(item.retail_price)} />
                    <DataRow label="Cost Price" value={formatCurrency(item.cost_price)} />
                    <DataRow label="Min Bid" value={formatCurrency(item.min_price)} emphasized />
                </SectionBlock>
                
                <SectionBlock title="Product Info">
                    <DataRow label="Brand" value={item.extracted_brand} />
                    <DataRow label="Model" value={item.extracted_model} />
                    <DataRow label="SKU" value={item.sku_extracted} mono />
                    <DataRow label="Source" value={item.source} />
                    <DataRow label="Condition" value={item.condition} />
                    <DataRow label="Quantity" value={String(item.quantity)} />
                    <DataRow label="Category" value={item.category} />
                </SectionBlock>

                {item.auction_id && (
                    <SectionBlock title="Auction">
                        <DataRow label="Auction ID" value={item.auction_id} mono />
                        <DataRow label="Listed At" value={item.listed_at ? formatDate(item.listed_at) : '—'} />
                        <DataRow label="Sold At" value={item.sold_at ? formatDate(item.sold_at) : '—'} />
                    </SectionBlock>
                )}

                <SectionBlock title="System">
                    <DataRow label="Item ID" value={item.id} mono />
                    <DataRow label="Created" value={formatDate(item.created_at)} />
                </SectionBlock>
            </DialogContent>
        </Dialog>
    );
}

// Вспомогательные компоненты (в том же файле)
function SectionBlock({ title, children }: { title: string; children: React.ReactNode }) {
    return (
        <div className="space-y-2">
            <Separator />
            <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">{title}</p>
            <div className="space-y-1.5">{children}</div>
        </div>
    );
}

function DataRow({ label, value, mono, emphasized }: { label: string; value?: string | null; mono?: boolean; emphasized?: boolean }) {
    if (!value) return null; // Не рендерить пустые поля
    return (
        <div className="flex justify-between items-center text-sm">
            <span className="text-muted-foreground">{label}</span>
            <span className={cn(
                mono && 'font-mono text-xs',
                emphasized && 'font-semibold text-emerald-600'
            )}>
                {value}
            </span>
        </div>
    );
}
```

### 3.2 Подключить в `InventoryPage.tsx`

Добавить состояние:
```tsx
const [selectedItem, setSelectedItem] = useState<InventoryItem | null>(null);
const [detailOpen, setDetailOpen] = useState(false);
```

Включить диалог в JSX:
```tsx
<InventoryItemDetailDialog
    item={selectedItem}
    open={detailOpen}
    onOpenChange={setDetailOpen}
/>
```

Убрать `disabled` с `View Details`:
```tsx
// Было:
<DropdownMenuItem disabled title="Coming soon">
    View Details
</DropdownMenuItem>

// Стало:
<DropdownMenuItem
    onClick={() => {
        setSelectedItem(item);
        setDetailOpen(true);
    }}
>
    View Details
</DropdownMenuItem>
```

---

## Изменение #4 — ImportPage: навигация и валидация

### 4.1 Раскомментировать "View Inventory" и подключить навигацию

```tsx
// Было:
{/* <Button>View Inventory</Button> -> Navigate? */}

// Стало:
<Button onClick={() => navigate('/inventory')}>
    View Inventory
</Button>
```

Добавить `useNavigate` из `react-router-dom`:
```tsx
import { useNavigate } from 'react-router-dom';
const navigate = useNavigate();
```

### 4.2 Добавить шаг CSV-валидации перед импортом

`validate_csv` Tauri-команда уже зарегистрирована. Вызывать её после выбора файла, ДО показа кнопки "Process Import":

**Новый API-метод** (проверить, есть ли уже в `api.ts`):
```ts
validateCsv: (filePath: string) =>
    invokeCommand<ValidationResult>('validate_csv', { filePath }),
```

**Новый тип** в `types/index.ts`:
```ts
export interface ValidationResult {
    valid: boolean;
    message: string;
    warnings: string[];
}
```

**Mock:**
```ts
case 'validate_csv':
    return { valid: true, message: 'CSV is valid. Checked 5 rows.', warnings: [] };
```

**В `ImportPage.tsx`** — добавить состояние и вызов после `handleSelectFile`:
```tsx
const [validation, setValidation] = useState<ValidationResult | null>(null);
const [validating, setValidating] = useState(false);

const handleSelectFile = async () => {
    const selected = await api.selectFile([{ name: 'CSV', extensions: ['csv'] }]);
    if (selected && typeof selected === 'string') {
        setFile(selected);
        setResult(null);
        setError(null);
        setValidation(null);
        setProgress(0);
        
        // Запустить валидацию сразу после выбора
        setValidating(true);
        try {
            const v = await api.validateCsv(selected);
            setValidation(v);
        } catch {
            // Молча игнорировать — не блокировать импорт
        } finally {
            setValidating(false);
        }
    }
};
```

**Отображение результата валидации** (между карточкой с файлом и кнопкой "Process Import"):
```tsx
{validating && (
    <p className="text-sm text-muted-foreground text-center animate-pulse">Validating CSV format...</p>
)}

{validation && !validating && (
    <div className={cn(
        "p-3 rounded-lg border text-sm flex items-start gap-2",
        validation.valid
            ? "bg-emerald-50 border-emerald-200 text-emerald-700"
            : "bg-red-50 border-red-200 text-red-700"
    )}>
        {validation.valid ? <Check className="h-4 w-4 mt-0.5 shrink-0" /> : <AlertCircle className="h-4 w-4 mt-0.5 shrink-0" />}
        <div>
            <p className="font-medium">{validation.message}</p>
            {validation.warnings.length > 0 && (
                <ul className="mt-1 space-y-0.5 text-xs opacity-80">
                    {validation.warnings.map((w, i) => <li key={i}>⚠ {w}</li>)}
                </ul>
            )}
        </div>
    </div>
)}
```

Кнопку "Process Import" показывать только если `file && !importing && !result` И `validation?.valid !== false` (т.е. либо валидация не проводилась, либо прошла):
```tsx
{file && !importing && !result && validation?.valid !== false && (
    <Button size="lg" onClick={handleImport}>Process Import</Button>
)}
```

---

## Изменение #5 — ReconciliationPage: заменить нативный `<select>` на Radix Select

`@radix-ui/react-select` уже установлен в `package.json`. Нативный `<select>` выглядит как инородный элемент в интерфейсе.

В `ReconciliationPage.tsx` заменить:
```tsx
<select className="flex h-10 w-full rounded-md border ..." ...>
    <option value="">Select an auction...</option>
    {auctions.map(a => <option key={a.id} value={a.id}>...</option>)}
</select>
```

На:
```tsx
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';

<Select value={selectedAuctionId} onValueChange={setSelectedAuctionId}>
    <SelectTrigger>
        <SelectValue placeholder="Select an auction..." />
    </SelectTrigger>
    <SelectContent>
        {auctions.map(a => (
            <SelectItem key={a.id} value={a.id}>
                {a.name} — {a.status} · {a.total_lots} {a.total_lots === 1 ? 'lot' : 'lots'}
            </SelectItem>
        ))}
    </SelectContent>
</Select>
```

---

## Список файлов и изменений

| Файл | Изменение |
|---|---|
| `src-tauri/src/auctions.rs` | Добавить `unassign_item` команду |
| `src-tauri/src/main.rs` | Зарегистрировать `auctions::unassign_item` |
| `src/lib/api.ts` | Добавить `unassignItem`, `validateCsv`; mock-записи для обоих |
| `src/types/index.ts` | Добавить `ValidationResult` |
| `src/pages/AuctionDetailPage.tsx` | Подключить `api.unassignItem` на кнопку `×` |
| `src/pages/AuctionsPage.tsx` | Исправить `handleExport` (saveFile + toast); заменить пустой grid на KPI-карточки |
| `src/pages/InventoryPage.tsx` | Подключить `InventoryItemDetailDialog`; убрать `disabled` с View Details |
| `src/components/inventory/InventoryItemDetailDialog.tsx` | **Создать новый** |
| `src/pages/ImportPage.tsx` | Раскомментировать View Inventory + useNavigate; добавить validateCsv шаг |
| `src/pages/ReconciliationPage.tsx` | Заменить `<select>` на Radix Select |

---

## Что НЕ меняется в этом блоке

- `ReportsPage.tsx` — не трогать.
- `DashboardPage.tsx` — не трогать.
- `SettingsPage.tsx` — не трогать.
- Sidebar, layout, роутинг — не трогать.
- Rust-модули `reconciliation.rs`, `nlp.rs`, `pricing.rs`, `hibid.rs` — не трогать.
- БД-схема — не меняется.
- Детальная страница `AuctionDetailPage` — только `×` кнопка.

---

## Критерии приёмки

- [ ] Кнопка `×` в Assigned Items на `AuctionDetailPage` убирает лот из аукциона, статус меняется на `InStock`, страница перезагружается без full-reload.
- [ ] После unassign счётчик `total_lots` в `AuctionsPage` и виджете Dashboard уменьшается на 1.
- [ ] Кнопка `CSV` в строке таблицы `AuctionsPage` открывает **диалог сохранения** и показывает toast об успехе.
- [ ] Пустой grid в `AuctionsPage` заменён тремя KPI-карточками (Draft / Active / Completed).
- [ ] `View Details` в dropdown `InventoryPage` открывает `InventoryItemDetailDialog`.
- [ ] Диалог показывает все непустые поля товара без дополнительных API-запросов.
- [ ] После успешного импорта в `ImportPage` кнопка "View Inventory" переводит на `/inventory`.
- [ ] После выбора CSV в `ImportPage` появляется статус валидации (valid / invalid + warnings).
- [ ] При невалидном CSV кнопка "Process Import" не отображается.
- [ ] Dropdown выбора аукциона на `ReconciliationPage` — Radix Select компонент.
- [ ] `tsc -b` — без ошибок.
- [ ] `npm run tauri dev` — без ошибок в консоли.
