const AUCTION_PREFIX = 'Sugarland';

function sanitizeFileNamePart(name: string): string {
    // Windows-forbidden characters: \ / : * ? " < > |
    return name.replace(/[\\/:*?"<>|]/g, ' ').replace(/\s+/g, ' ').trim();
}

export function extractAuctionNumber(input: string): string | null {
    const matches = input.match(/\d+/g);
    if (!matches || matches.length === 0) return null;
    const last = matches[matches.length - 1];
    const parsed = Number.parseInt(last, 10);
    if (!Number.isFinite(parsed) || parsed <= 0) return null;
    return String(parsed);
}

export function buildAuctionNameFromNumber(input: string): string | null {
    const number = extractAuctionNumber(input);
    if (!number) return null;
    return `${AUCTION_PREFIX} ${number}`;
}

export function buildManagerReportSheetName(auctionName: string): string {
    const number = extractAuctionNumber(auctionName);
    return `S-${number ?? '0'}`;
}

export function buildManagerReportFileName(auctionName: string): string {
    const number = extractAuctionNumber(auctionName);
    if (number) {
        return `${AUCTION_PREFIX}_${number} min prices.xlsx`;
    }
    const safeAuctionName = sanitizeFileNamePart(auctionName) || AUCTION_PREFIX;
    return `${safeAuctionName} min prices.xlsx`;
}
