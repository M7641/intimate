import type { Table } from "@tanstack/react-table";

import {
  Pagination,
  PaginationContent,
  PaginationEllipsis,
  PaginationItem,
  PaginationLink,
  PaginationNext,
  PaginationPrevious,
} from "@/components/ui/pagination";

export default function TemplatePagination<T>({
  table,
  justifyContent = "justify-end",
}: {
  table: Table<T>;
  justifyContent?: string;
}): React.ReactElement {
  const currentPage = table.getState().pagination.pageIndex;
  const pageCount = table.getPageCount();
  const isFirstPage = currentPage === 0;
  const isLastPage = currentPage === pageCount - 1;

  return (
    <Pagination className={justifyContent}>
      <PaginationContent>
        <PaginationItem>
          <PaginationPrevious
            onClick={() => (!isFirstPage ? table.previousPage() : undefined)}
            className={isFirstPage ? "opacity-50 pointer-events-none" : ""}
          />
        </PaginationItem>
        {Array.from({ length: pageCount }).map((_, i) => {
          if (
            i === 0 ||
            i === pageCount - 1 ||
            Math.abs(i - currentPage) <= 1 ||
            pageCount <= 5
          ) {
            return (
              <PaginationItem key={i}>
                <PaginationLink
                  onClick={() => table.setPageIndex(i)}
                  isActive={currentPage === i}
                >
                  {i + 1}
                </PaginationLink>
              </PaginationItem>
            );
          }
          // Show ellipses before/after the current page range.
          if (
            (i === 1 && currentPage > 2) ||
            (i === pageCount - 2 && currentPage < pageCount - 5)
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
            onClick={() => (!isLastPage ? table.nextPage() : undefined)}
            className={isLastPage ? "opacity-50 pointer-events-none" : ""}
          />
        </PaginationItem>
      </PaginationContent>
      <span className="text-xs text-muted-foreground ml-2">
        Page {currentPage + 1} of {pageCount}
      </span>
    </Pagination>
  );
}
