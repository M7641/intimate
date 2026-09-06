
import React from 'react';
import TemplatePagination from '@/components/TemplatePagination.tsx';
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
    }: {
        data: T[],
        columns: any,
        pagination?: boolean,
        paginationSize?: number,
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

    return (
        <div>
            {typeof pagination === 'boolean' && !pagination ? null : (
                <TemplatePagination
                    table={table}
                    justifyContent="justify-end"
                />
            )}
            <table className="min-w-full divide-y">
                <thead className="border-b-0">
                    {table.getHeaderGroups().map(headerGroup => (
                        <tr key={headerGroup.id}>
                            {headerGroup.headers.map(header => (
                                <th
                                    key={header.id}
                                    className="px-2 py-2 text-[12px] font-semibold tracking-wider"
                                    style={{
                                        cursor: header.column.getCanSort() ? "pointer" : "default",
                                        width: `${header.getSize()}px`
                                    }}
                                    onClick={header.column.getCanSort() ? () => header.column.toggleSorting(): undefined}
                                >
                                    <div className="flex items-center justify-center">
                                        <div className="ml-[16px]">
                                            {flexRender(header.column.columnDef.header, header.getContext())}
                                        </div>
                                        <div className="ml-[2px] text-[10px] w-[14px]">
                                            {header.column.getCanSort() === true && (
                                                header.column.getIsSorted() !== 'asc'
                                                && header.column.getIsSorted() !== 'desc'
                                            ) ?
                                                <FaSort size={14} />: ''
                                            }
                                            {
                                            header.column.getIsSorted() === 'asc' ?
                                                <FaSortDown size={14} />: ''
                                            }
                                            {
                                            header.column.getIsSorted() === 'desc' ?
                                                <FaSortUp size={14} />: ''
                                            }
                                        </div>
                                    </div>
                                </th>
                            ))}
                        </tr>
                    ))}
                </thead>
                <tbody className="divide-y">
                    {table.getRowModel().rows.map(row => (
                        <tr key={row.id} style={{ height: '40px' }}>
                            {row.getVisibleCells().map(cell => (
                                <td
                                    key={cell.id}
                                    className="whitespace-nowrap text-[14px] text-center px-2"
                                >
                                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                                </td>
                            ))}
                        </tr>
                    ))}
                </tbody>
            </table>
        </div>
    );
}
