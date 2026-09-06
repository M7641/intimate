import { useReactTable } from '@tanstack/react-table';

import {
  Pagination,
  PaginationContent,
  PaginationEllipsis,
  PaginationItem,
  PaginationLink,
  PaginationNext,
  PaginationPrevious,
} from "@/components/ui/pagination.tsx";

export default function TemplatePagination<T>({
  table,
  justifyContent = "justify-end",
}: {
  table: ReturnType<typeof useReactTable<T>>,
  justifyContent?: string
}): React.ReactElement {
  "use no memo";
  return (
    <>
      <Pagination className={`${justifyContent}`}>
        <PaginationContent>
          <PaginationItem>
            <PaginationPrevious
              onClick={() => table.getState().pagination.pageIndex !== 0 ? table.previousPage() : undefined}
              style={{ color: table.getState().pagination.pageIndex !== 0 ? undefined : 'grey' }}
            />
          </PaginationItem>
          {Array.from({ length: table.getPageCount() }).map((_, i) => {
            const pageCount = table.getPageCount();

            if (i === 0 ||
                i === pageCount - 1 ||
                Math.abs(i - table.getState().pagination.pageIndex) <= 1 ||
                pageCount <= 5
            ) {
              return (
                <PaginationItem key={i}>
                  <PaginationLink
                    onClick={() => table.setPageIndex(i)}
                    isActive={table.getState().pagination.pageIndex === i}
                  >
                    {i + 1}
                  </PaginationLink>
                </PaginationItem>
              );
            }
            // Show ellipses before/after current page range
            if (
              (i === 1 && table.getState().pagination.pageIndex > 2) ||
              (i === pageCount - 2 && table.getState().pagination.pageIndex < pageCount - 5)
            ) {
              return (
                <PaginationItem key={i}>
                  <PaginationEllipsis />
                </PaginationItem>
              );
            }
            return null;
          })}
          <PaginationItem>
            <PaginationNext
                onClick={
                    () => table.getState().pagination.pageIndex !== table.getPageCount() - 1 ?
                    table.nextPage() :
                    undefined
                }
                style={{
                    color: table.getState().pagination.pageIndex !== table.getPageCount() - 1 ? undefined : 'grey'
                 }}
            />
          </PaginationItem>
        </PaginationContent>
      </Pagination>
    </>
  )
}
