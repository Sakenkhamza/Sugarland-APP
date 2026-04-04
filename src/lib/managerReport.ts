import * as XLSX from 'xlsx-js-style';
import type { InventoryItem } from '@/types';
import { buildManagerReportSheetName } from '@/lib/auctionNaming';

type VendorCostMap = Record<string, number>;
type ConditionMarginMap = Record<string, Record<string, number>>;
type RepeaterStatsMap = Record<string, Record<string, number>>;
type SaleOrderIndex = Record<string, number>;

export interface ManagerReportOptions {
    items: InventoryItem[];
    auctionName?: string;
    vendorCosts?: VendorCostMap;
    conditionMarginsBySupplier?: ConditionMarginMap;
    historyHeaders?: string[];
    repeaterStatsByTitle?: RepeaterStatsMap;
}

export interface ManagerReportPreviewRow {
    item: InventoryItem;
    lotNumber: string;
    readDescriptionFlag: boolean;
    saleOrder: number | '';
    title: string;
    retailPrice: number;
    source: string;
    condition: string;
    costPct: number;
    minPricePct: number;
    costPrice: number;
    minPriceHalf: number;
    minPriceOne: number;
    repeat: number;
    historyBySeason: Record<string, number>;
}

export const HISTORY_HEADERS = ['S20', 'S21', 'S22', 'S23', 'S24', 'S25', 'S26', 'S27'] as const;
const REPEAT_HEADER = '\u041f\u043e\u0432\u0442\u043e\u0440'; // Repeat
const FLAGGED_LOT_FILL_COLOR = 'FFF4B183';

function round2(value: number): number {
    return Math.round(value * 100) / 100;
}

function extractLastNumber(value: string): number | null {
    const matches = value.match(/\d+/g);
    if (!matches || matches.length === 0) return null;
    const parsed = Number(matches[matches.length - 1]);
    if (!Number.isFinite(parsed) || parsed <= 0) return null;
    return Math.floor(parsed);
}

export function getHistoryHeadersForAuction(auctionName?: string): string[] {
    const auctionNumber = extractLastNumber(auctionName || '');
    const windowSize = HISTORY_HEADERS.length;
    if (!auctionNumber || auctionNumber < 20) return [...HISTORY_HEADERS];
    const start = Math.max(auctionNumber - (windowSize - 1), 1);
    return Array.from({ length: windowSize }, (_, idx) => `S${start + idx}`);
}

function naturalLotCompare(a: string, b: string): number {
    const chunkPattern = /(\d+)|(\D+)/g;
    const aParts = (a || '').match(chunkPattern) || [];
    const bParts = (b || '').match(chunkPattern) || [];

    for (let idx = 0; idx < Math.max(aParts.length, bParts.length); idx += 1) {
        const aPart = aParts[idx] || '';
        const bPart = bParts[idx] || '';
        const aNum = Number.parseInt(aPart, 10);
        const bNum = Number.parseInt(bPart, 10);

        if (!Number.isNaN(aNum) && !Number.isNaN(bNum)) {
            if (aNum !== bNum) return aNum - bNum;
            continue;
        }

        const normalizedA = aPart.toLowerCase();
        const normalizedB = bPart.toLowerCase();
        if (normalizedA < normalizedB) return -1;
        if (normalizedA > normalizedB) return 1;
    }

    return 0;
}

export function buildManagerReportSaleOrderIndex(items: InventoryItem[]): SaleOrderIndex {
    const natural = [...items].sort((a, b) => (
        naturalLotCompare(a.lot_number || '', b.lot_number || '')
        || a.id.localeCompare(b.id)
    ));

    return natural.reduce<SaleOrderIndex>((result, item, index) => {
        result[item.id] = index + 1;
        return result;
    }, {});
}

function deriveCostPct(item: InventoryItem, vendorCosts: VendorCostMap): number {
    const source = item.source || '';
    const sourceCost = vendorCosts[source];
    if (Number.isFinite(sourceCost)) return Math.max(0, sourceCost);
    if (!item.retail_price) return 0;
    return Math.max(0, round2((item.cost_price || 0) / item.retail_price));
}

function deriveMarginPct(item: InventoryItem, conditionMarginsBySupplier: ConditionMarginMap): number {
    const source = item.source || '';
    const condition = item.condition || '';
    const sourceMargins = conditionMarginsBySupplier[source];
    const sourceMargin = sourceMargins?.[condition];
    if (Number.isFinite(sourceMargin)) return Math.max(0, sourceMargin);
    if (!item.retail_price) return 0.1;
    const derived = ((item.min_price || 0) - (item.cost_price || 0)) / item.retail_price;
    if (!Number.isFinite(derived)) return 0.1;
    return Math.max(0, round2(derived));
}

function setCellNumberFormat(worksheet: XLSX.WorkSheet, cellAddress: string, format: string) {
    const cell = worksheet[cellAddress];
    if (cell) cell.z = format;
}

function setCellFill(worksheet: XLSX.WorkSheet, cellAddress: string, fillColor: string) {
    const cell = worksheet[cellAddress] as (XLSX.CellObject & {
        s?: {
            fill?: {
                patternType?: string;
                fgColor?: { rgb?: string };
            };
        };
    }) | undefined;
    if (!cell) return;

    cell.s = {
        ...(cell.s || {}),
        fill: {
            patternType: 'solid',
            fgColor: { rgb: fillColor },
        },
    };
}

function applyFormats(worksheet: XLSX.WorkSheet, lastRow: number, historyHeaders: string[]) {
    const percentCols = ['G', 'H'];
    const numberColIndexes = [3, 8, 9, 10, 12, ...historyHeaders.map((_, idx) => 13 + idx)];
    const numberCols = numberColIndexes.map((colIndex) => XLSX.utils.encode_col(colIndex));
    for (let row = 2; row <= lastRow; row += 1) {
        percentCols.forEach((col) => setCellNumberFormat(worksheet, `${col}${row}`, '0%'));
        numberCols.forEach((col) => setCellNumberFormat(worksheet, `${col}${row}`, '#,##0'));
    }
}

function applyFlaggedLotStyles(worksheet: XLSX.WorkSheet, previewRows: ManagerReportPreviewRow[]) {
    previewRows.forEach((rowData, index) => {
        if (!rowData.readDescriptionFlag) return;
        setCellFill(worksheet, `A${index + 2}`, FLAGGED_LOT_FILL_COLOR);
    });
}

export function buildManagerReportPreviewRows(options: ManagerReportOptions): ManagerReportPreviewRow[] {
    const {
        items,
        vendorCosts = {},
        conditionMarginsBySupplier = {},
        historyHeaders = getHistoryHeadersForAuction(options.auctionName),
        repeaterStatsByTitle = {},
    } = options;
    const derivedSaleOrderByItemId = buildManagerReportSaleOrderIndex(items);

    return items.map((item) => {
        const costPct = deriveCostPct(item, vendorCosts);
        const minPricePct = deriveMarginPct(item, conditionMarginsBySupplier);
        const retailPrice = Math.round(item.retail_price || 0);
        const costPrice = round2(retailPrice * costPct);
        const minPriceHalf = costPrice + retailPrice * minPricePct / 2;
        const minPriceOne = costPrice + retailPrice * minPricePct;

        const titleHistory = item.normalized_title ? repeaterStatsByTitle[item.normalized_title] : undefined;
        const historyBySeason = Object.fromEntries(
            historyHeaders.map((header) => {
                const rawValue = titleHistory?.[header];
                return [header, Number.isFinite(rawValue) ? Number(rawValue) : 0];
            }),
        );
        const repeat = historyHeaders.length > 0
            ? Math.max(...historyHeaders.map((header) => historyBySeason[header] ?? 0))
            : 0;

        return {
            item,
            lotNumber: item.lot_number || '',
            readDescriptionFlag: Boolean(item.read_description_flag),
            saleOrder: typeof item.sale_order === 'number'
                ? item.sale_order
                : (derivedSaleOrderByItemId[item.id] ?? ''),
            title: item.raw_title || '',
            retailPrice,
            source: item.source || '',
            condition: item.condition || '',
            costPct,
            minPricePct,
            costPrice,
            minPriceHalf,
            minPriceOne,
            repeat,
            historyBySeason,
        };
    });
}

export function buildManagerReportWorkbook(options: ManagerReportOptions): XLSX.WorkBook {
    const { auctionName = '' } = options;
    const historyHeaders = options.historyHeaders?.length
        ? options.historyHeaders
        : getHistoryHeadersForAuction(auctionName);
    const previewRows = buildManagerReportPreviewRows({
        ...options,
        historyHeaders,
    });

    const rows: Array<Array<string | number | XLSX.CellObject | null>> = [];
    rows.push([
        'LotNumber',
        'sale order',
        'Title',
        'Retail Price',
        'Source',
        'Condition',
        'cost',
        'min price %',
        'cost price',
        'min pr (+0,5)',
        'min pr (+1)',
        null,
        REPEAT_HEADER,
        ...historyHeaders,
    ]);

    previewRows.forEach((rowData, idx) => {
        const excelRow = idx + 2;
        const historyStartColIndex = 13; // N
        const historyEndColIndex = historyStartColIndex + historyHeaders.length - 1;
        const historyCells = historyHeaders.map((seasonHeader) => ({
            t: 'n',
            v: rowData.historyBySeason[seasonHeader] ?? 0,
        }) as XLSX.CellObject);

        const repeatCell: XLSX.CellObject = historyHeaders.length > 0
            ? {
                t: 'n',
                v: rowData.repeat,
                f: `MAX(${XLSX.utils.encode_col(historyStartColIndex)}${excelRow}:${XLSX.utils.encode_col(historyEndColIndex)}${excelRow})`,
            }
            : { t: 'n', v: 0 };

        rows.push([
            rowData.lotNumber,
            rowData.saleOrder,
            rowData.title,
            rowData.retailPrice,
            rowData.source,
            rowData.condition,
            rowData.costPct,
            rowData.minPricePct,
            { t: 'n', v: rowData.costPrice, f: `ROUND(D${excelRow}*G${excelRow}, 2)` },
            { t: 'n', v: rowData.minPriceHalf, f: `I${excelRow}+$D${excelRow}*H${excelRow}/2` },
            { t: 'n', v: rowData.minPriceOne, f: `$I${excelRow}+$D${excelRow}*$H${excelRow}` },
            null,
            repeatCell,
            ...historyCells,
        ]);
    });

    const worksheet = XLSX.utils.aoa_to_sheet(rows);
    const baseColumns: XLSX.ColInfo[] = [
        { wch: 8.33 }, { wch: 8.33 }, { wch: 32.89 }, { wch: 8.33 }, { wch: 13.56 },
        { wch: 8.78 }, { wch: 8.33 }, { wch: 10.22 }, { wch: 8.33 }, { wch: 13.56 },
        { wch: 11.89 }, { wch: 11.89 }, { wch: 8.33 },
    ];
    worksheet['!cols'] = [
        ...baseColumns,
        ...historyHeaders.map(() => ({ wch: 8.33 })),
    ];

    const lastRow = Math.max(rows.length, 1);
    const lastColumnIndex = 12 + historyHeaders.length;
    worksheet['!autofilter'] = { ref: `A1:${XLSX.utils.encode_col(lastColumnIndex)}${lastRow}` };
    applyFormats(worksheet, lastRow, historyHeaders);
    applyFlaggedLotStyles(worksheet, previewRows);

    const workbook = XLSX.utils.book_new();
    XLSX.utils.book_append_sheet(workbook, worksheet, buildManagerReportSheetName(auctionName));
    return workbook;
}
