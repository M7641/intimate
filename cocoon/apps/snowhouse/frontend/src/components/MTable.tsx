import React from 'react';
import NoDataAvailable from '@/components/NoDataAvailable';
import { cn } from '@/lib/utils';
import {
    flexRender,
    getCoreRowModel,
    useReactTable,
    getSortedRowModel,
} from '@tanstack/react-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import { FaSort, FaSortUp, FaSortDown } from "react-icons/fa";

type ColumnSort = {
  id: string
  desc: boolean
}

/** Body row height (matches the `h-10` on each row). Rows never wrap
 * (`whitespace-nowrap`), so this is exact and lets the virtualizer size the
 * scroll window without measuring each row. */
const ROW_HEIGHT = 40;

export default function MTable<T>(
    {
        data,
        columns,
        maxHeight = "70vh",
    }: {
        data: T[],
        columns: any,
        /** Cap the scroll container; beyond it the body scrolls and virtualizes.
         * Smaller tables simply render short and never scroll. */
        maxHeight?: string,
    }
) {
    "use no memo";

    const [sorting, setSorting] = React.useState<ColumnSort[]>([])
    const table = useReactTable({
        data: data,
        columns: columns,
        getCoreRowModel: getCoreRowModel(),
        getSortedRowModel: getSortedRowModel(),
        state: { sorting },
        onSortingChange: setSorting,
    });

    const rows = table.getRowModel().rows;

    // Virtualize the body: only the rows in (and just around) the viewport are in
    // the DOM, so a 1,000-row result renders as cheaply as a 20-row one. Rows are
    // kept in the native <table> via top/bottom spacer rows, so column widths and
    // alignment still come from the browser's table layout.
    const scrollRef = React.useRef<HTMLDivElement>(null);
    const virtualizer = useVirtualizer({
        count: rows.length,
        getScrollElement: () => scrollRef.current,
        estimateSize: () => ROW_HEIGHT,
        overscan: 12,
    });

    if (data.length === 0) {
        return <NoDataAvailable />;
    }

    const virtualRows = virtualizer.getVirtualItems();
    const leafColumns = table.getVisibleLeafColumns();
    const colCount = leafColumns.length;
    // Pin the table to the exact sum of its column sizes. Combined with the
    // <colgroup> below and table-fixed, this removes every degree of freedom the
    // browser has to redistribute width — so the layout is byte-identical in
    // every state (scrolled or not, loading or loaded, scrollbar or none).
    const totalWidth = leafColumns.reduce((sum, col) => sum + col.getSize(), 0);
    const paddingTop = virtualRows.length ? virtualRows[0].start : 0;
    const paddingBottom = virtualRows.length
        ? virtualizer.getTotalSize() - virtualRows[virtualRows.length - 1].end
        : 0;

    return (
        <div ref={scrollRef} className="overflow-auto" style={{ maxHeight }}>
            {/* table-fixed + explicit width + <colgroup>: column widths come only
             * from the col definitions below, never from cell content or the
             * container. Without the explicit width, `min-w-full` let the browser
             * flip between natural width (overflow) and stretched-to-100%, so the
             * table changed shape whenever the container width shifted. */}
            <table className="table-fixed divide-y" style={{ width: totalWidth }}>
                <colgroup>
                    {leafColumns.map(col => (
                        <col key={col.id} style={{ width: `${col.getSize()}px` }} />
                    ))}
                </colgroup>
                <thead className="sticky top-0 z-10 bg-card border-b-0">
                    {table.getHeaderGroups().map(headerGroup => (
                        <tr key={headerGroup.id}>
                            {headerGroup.headers.map(header => {
                                const align = (header.column.columnDef.meta as any)?.align;
                                const canSort = header.column.getCanSort();
                                const sorted = header.column.getIsSorted();
                                const label = flexRender(header.column.columnDef.header, header.getContext());
                                const indicator = (
                                    <span className="ml-[2px] text-[10px] w-[14px] inline-flex justify-center" aria-hidden="true">
                                        {sorted === 'asc' && <FaSortUp size={14} />}
                                        {sorted === 'desc' && <FaSortDown size={14} />}
                                        {canSort && !sorted && <FaSort size={14} className="opacity-40" />}
                                    </span>
                                );
                                return (
                                    <th
                                        key={header.id}
                                        scope="col"
                                        aria-sort={
                                            !canSort ? undefined
                                            : sorted === 'asc' ? 'ascending'
                                            : sorted === 'desc' ? 'descending'
                                            : 'none'
                                        }
                                        className={cn(
                                            "px-2 py-2 text-[12px] font-semibold tracking-wider",
                                            align === "right" ? "text-right" : "text-center"
                                        )}
                                        style={{ width: `${header.getSize()}px` }}
                                    >
                                        {canSort ? (
                                            <button
                                                type="button"
                                                onClick={() => header.column.toggleSorting()}
                                                className={cn(
                                                    "inline-flex items-center w-full rounded-sm transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                                                    align === "right" ? "justify-end" : "justify-center"
                                                )}
                                            >
                                                <span className={align === "right" ? "" : "ml-[16px]"}>{label}</span>
                                                {indicator}
                                            </button>
                                        ) : (
                                            <div className={cn(
                                                "flex items-center",
                                                align === "right" ? "justify-end" : "justify-center"
                                            )}>
                                                {/* No sort chevron here, so no ml offset to balance:
                                                 * the label is truly centred and lines up with a
                                                 * centred cell (e.g. the Full Query button). */}
                                                <span>{label}</span>
                                            </div>
                                        )}
                                    </th>
                                );
                            })}
                        </tr>
                    ))}
                </thead>
                <tbody className="divide-y">
                    {paddingTop > 0 && (
                        <tr aria-hidden="true">
                            <td colSpan={colCount} style={{ height: paddingTop, padding: 0, border: 0 }} />
                        </tr>
                    )}
                    {virtualRows.map(virtualRow => {
                        const row = rows[virtualRow.index];
                        return (
                            <tr key={row.id} className="h-10 hover:bg-muted/50 transition-colors">
                                {row.getVisibleCells().map(cell => {
                                    const align = (cell.column.columnDef.meta as any)?.align;
                                    return (
                                        <td
                                            key={cell.id}
                                            className={cn(
                                                // overflow-hidden clips long values to the now-fixed
                                                // column width so they can't spill into the neighbour.
                                                "overflow-hidden text-ellipsis whitespace-nowrap text-[14px] px-2",
                                                align === "right" ? "text-right" : "text-center"
                                            )}
                                        >
                                            {flexRender(cell.column.columnDef.cell, cell.getContext())}
                                        </td>
                                    );
                                })}
                            </tr>
                        );
                    })}
                    {paddingBottom > 0 && (
                        <tr aria-hidden="true">
                            <td colSpan={colCount} style={{ height: paddingBottom, padding: 0, border: 0 }} />
                        </tr>
                    )}
                </tbody>
            </table>
        </div>
    );
}
