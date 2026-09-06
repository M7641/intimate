import React from 'react';
import TemplatePagination from '@/components/TemplatePagination.tsx';
import NoDataAvailable from '@/components/NoDataAvailable';
import { cn } from '@/lib/utils';
import {
    flexRender,
    getCoreRowModel,
    useReactTable,
    getPaginationRowModel,
    getSortedRowModel,
} from '@tanstack/react-table';
import { FaSort, FaSortUp, FaSortDown } from "react-icons/fa";

type ColumnSort = {
  id: string
  desc: boolean
}


export default function MTable<T>(
    {
        data,
        columns,
        pagination = false,
        paginationSize = 10,
        stickyHeader = false,
    }: {
        data: T[],
        columns: any,
        pagination?: boolean,
        paginationSize?: number,
        stickyHeader?: boolean,
    }
) {
    "use no memo";

    const [sorting, setSorting] = React.useState<ColumnSort[]>([])
    const table = useReactTable({
        data: data,
        columns: columns,
        getCoreRowModel: getCoreRowModel(),
        getPaginationRowModel: getPaginationRowModel(),
        getSortedRowModel: getSortedRowModel(),
        initialState: {
            pagination: {
                pageSize: paginationSize,
            },
        },
        state: { sorting },
        onSortingChange: setSorting,
    });

    if (data.length === 0) {
        return <NoDataAvailable />;
    }

    return (
        <div>
            <div className="overflow-x-auto">
            <table className="min-w-full divide-y">
                <thead className={cn("border-b-0", stickyHeader && "sticky top-0 z-10 bg-card")}>
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
                                                <span className={align === "right" ? "" : "ml-[16px]"}>{label}</span>
                                            </div>
                                        )}
                                    </th>
                                );
                            })}
                        </tr>
                    ))}
                </thead>
                <tbody className="divide-y">
                    {table.getRowModel().rows.map(row => (
                        <tr key={row.id} className="h-10 hover:bg-muted/50 transition-colors">
                            {row.getVisibleCells().map(cell => {
                                const align = (cell.column.columnDef.meta as any)?.align;
                                return (
                                    <td
                                        key={cell.id}
                                        className={cn(
                                            "whitespace-nowrap text-[14px] px-2",
                                            align === "right" ? "text-right" : "text-center"
                                        )}
                                    >
                                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                                    </td>
                                );
                            })}
                        </tr>
                    ))}
                </tbody>
            </table>
            </div>
            {typeof pagination === 'boolean' && !pagination ? null : (
                <TemplatePagination
                    table={table}
                    justifyContent="justify-end"
                />
            )}
        </div>
    );
}
