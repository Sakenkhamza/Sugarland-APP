import { ArrowUpDown, ArrowUp, ArrowDown } from 'lucide-react';
import { TableHead } from '@/components/ui/table';
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { cn } from '@/lib/utils';

export interface SortConfig {
    column: string;
    direction: 'asc' | 'desc';
}

interface SortableTableHeadProps {
    column: string;
    label: string;
    sortConfig: SortConfig | null;
    onSort: (column: string, direction: 'asc' | 'desc') => void;
    className?: string;
    /** Whether this column contains text data (shows A→Z / Z→A labels) */
    isText?: boolean;
    /** Extra items to show in the dropdown menu */
    extraItems?: { label: string; onClick: () => void; isActive?: boolean }[];
}

export function SortableTableHead({
    column,
    label,
    sortConfig,
    onSort,
    className,
    isText = false,
    extraItems,
}: SortableTableHeadProps) {
    const isActive = sortConfig?.column === column;
    const activeDir = isActive ? sortConfig!.direction : null;

    return (
        <TableHead className={cn('p-0', className)}>
            <DropdownMenu>
                <DropdownMenuTrigger asChild>
                    <button
                        className={cn(
                            'flex items-center gap-1 w-full h-full px-2 py-2 text-left text-xs font-medium',
                            'hover:bg-muted/50 transition-colors cursor-pointer select-none',
                            'focus:outline-none focus-visible:ring-1 focus-visible:ring-ring',
                            isActive && 'text-foreground'
                        )}
                    >
                        <span className="truncate">{label}</span>
                        {isActive ? (
                            activeDir === 'asc' ? (
                                <ArrowUp className="h-3 w-3 shrink-0 text-primary" />
                            ) : (
                                <ArrowDown className="h-3 w-3 shrink-0 text-primary" />
                            )
                        ) : (
                            <ArrowUpDown className="h-3 w-3 shrink-0 opacity-30" />
                        )}
                    </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start" className="min-w-[200px]">
                    <DropdownMenuItem
                        onClick={() => onSort(column, 'asc')}
                        className={cn(
                            'gap-2',
                            activeDir === 'asc' && 'bg-accent font-medium'
                        )}
                    >
                        <ArrowUp className="h-4 w-4 text-muted-foreground" />
                        {isText ? 'Sort A to Z' : 'Sort Smallest to Largest'}
                    </DropdownMenuItem>
                    <DropdownMenuItem
                        onClick={() => onSort(column, 'desc')}
                        className={cn(
                            'gap-2',
                            activeDir === 'desc' && 'bg-accent font-medium'
                        )}
                    >
                        <ArrowDown className="h-4 w-4 text-muted-foreground" />
                        {isText ? 'Sort Z to A' : 'Sort Largest to Smallest'}
                    </DropdownMenuItem>

                    {extraItems && extraItems.length > 0 && (
                        <>
                            <div className="h-px bg-muted my-1" />
                            <div className="px-2 py-1.5 text-[10px] font-semibold text-muted-foreground uppercase tracking-wider">
                                Pin to top
                            </div>
                            {extraItems.map((item, idx) => (
                                <DropdownMenuItem
                                    key={idx}
                                    onClick={item.onClick}
                                    className={cn(
                                        'gap-2',
                                        item.isActive && 'bg-accent font-medium'
                                    )}
                                >
                                    <span className={cn(
                                        "h-2 w-2 rounded-full shrink-0",
                                        item.isActive ? "bg-primary" : "bg-muted"
                                    )} />
                                    {item.label}
                                </DropdownMenuItem>
                            ))}
                        </>
                    )}
                </DropdownMenuContent>
            </DropdownMenu>
        </TableHead>
    );
}
