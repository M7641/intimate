import * as React from "react";
import TemplatePagination from "@/components/TemplatePagination";
import NoDataAvailable from "@/components/NoDataAvailable";
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from "@/components/ui/hover-card";
import { cn } from "@/lib/utils";
import {
  flexRender,
  getCoreRowModel,
  useReactTable,
  getPaginationRowModel,
  getSortedRowModel,
  getFilteredRowModel,
  type ColumnDef,
  type SortingState,
  type Column,
  type Header,
  type Row,
} from "@tanstack/react-table";
import { FaSort, FaSortUp, FaSortDown } from "react-icons/fa";
import { FiSearch, FiX, FiBarChart2 } from "react-icons/fi";

/** Row height in px — must match the `h-10` applied to body rows. */
const ROW_HEIGHT = 40;

/** Number of bars in a column-shape strip. Numeric columns always fill all
 *  of them (one histogram bin each); categorical columns use as many as they
 *  have distinct values, up to this cap. */
const SHAPE_BARS = 28;

type ColumnShape =
  | {
      kind: "numeric";
      bins: number[];
      peak: number;
      min: number;
      max: number;
      count: number;
    }
  | { kind: "categorical"; bars: number[]; distinct: number; count: number }
  | { kind: "empty" };

const shapeFmt = new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 });

/** Derive the visual "shape" of a column from its loaded values. Numeric
 *  columns become a histogram; everything else becomes a frequency profile of
 *  its most common values. Pure and cheap enough to re-run as filters narrow
 *  the row set, which is what makes the strip morph in real time. */
function computeColumnShape(values: unknown[]): ColumnShape {
  let count = 0;
  let allNumeric = true;
  const nums: number[] = [];
  const freq = new Map<string, number>();
  for (const v of values) {
    if (v === null || v === undefined || v === "") continue;
    count++;
    const n = typeof v === "number" ? v : Number(v);
    if (Number.isFinite(n)) nums.push(n);
    else allNumeric = false;
    const key = String(v);
    freq.set(key, (freq.get(key) ?? 0) + 1);
  }
  if (count === 0) return { kind: "empty" };

  if (allNumeric) {
    let min = Infinity;
    let max = -Infinity;
    for (const n of nums) {
      if (n < min) min = n;
      if (n > max) max = n;
    }
    const span = max - min || 1;
    const bins = new Array(SHAPE_BARS).fill(0);
    for (const n of nums) {
      let i = Math.floor(((n - min) / span) * SHAPE_BARS);
      if (i >= SHAPE_BARS) i = SHAPE_BARS - 1;
      bins[i]++;
    }
    let peak = 0;
    for (const b of bins) if (b > peak) peak = b;
    return { kind: "numeric", bins, peak, min, max, count };
  }

  const bars = [...freq.values()].sort((a, b) => b - a).slice(0, SHAPE_BARS);
  return { kind: "categorical", bars, distinct: freq.size, count };
}

/** The distribution shown inside a column's hover popup: a histogram (numeric)
 *  or frequency profile (categorical), plus a one-line caption of the range or
 *  cardinality. Kept presentational — the shape is computed by the caller. */
function DistributionView({ shape }: { shape?: ColumnShape }) {
  // Normalize each bar to a 0–1 height. Numeric bars read against the tallest
  // bin (a classic histogram); categorical bars read against the row count, so
  // a dominant value stands tall and an all-unique column stays flat.
  const cells = (() => {
    const s = shape;
    if (!s || s.kind === "empty") return [];
    if (s.kind === "numeric") return s.bins.map((b) => (s.peak ? b / s.peak : 0));
    return s.bars.map((b) => (s.count ? b / s.count : 0));
  })();
  const caption = (() => {
    const s = shape;
    if (!s || s.kind === "empty") return "No values in view";
    if (s.kind === "numeric")
      return `${shapeFmt.format(s.min)} to ${shapeFmt.format(s.max)} · ${s.count.toLocaleString()} values`;
    return `${s.distinct.toLocaleString()} distinct · ${s.count.toLocaleString()} values`;
  })();
  const kind = shape?.kind === "numeric" ? "Distribution" : "Top values";

  if (cells.length === 0) {
    return <p className="text-xs text-muted-foreground">{caption}</p>;
  }

  return (
    <div className="space-y-1.5">
      <p className="text-[11px] font-medium text-muted-foreground">{kind}</p>
      <div className="flex h-12 items-end gap-px">
        {cells.map((ratio, i) => {
          const scale = Math.max(ratio, ratio > 0 ? 0.06 : 0);
          return (
            <div
              key={i}
              className="h-full flex-1 origin-bottom rounded-[1px] bg-primary"
              style={{
                opacity: 0.3 + ratio * 0.5,
                transform: `scaleY(${scale})`,
              }}
            />
          );
        })}
      </div>
      <p className="text-[11px] tabular-nums text-muted-foreground">{caption}</p>
    </div>
  );
}

export default function MTable<T>({
  data,
  columns,
  pagination = false,
  paginationSize = 10,
  searchable = false,
  fill = false,
  columnShapes = false,
  onRowClick,
}: {
  data: T[];
  columns: ColumnDef<T, any>[];
  /** When set, each body row becomes clickable and calls this with the row's
   *  data. Adds a pointer cursor and keyboard (Enter) activation. */
  onRowClick?: (row: T) => void;
  /** Opt into classic pagination instead of the default virtualised scroll. */
  pagination?: boolean;
  paginationSize?: number;
  /** Show an instant client-side filter over the loaded rows. */
  searchable?: boolean;
  /** Stretch to fill a height-constrained flex parent (e.g. a page pane).
   *  When false, the virtualised viewport caps itself at a fixed max height. */
  fill?: boolean;
  /** Render a live distribution strip under each header. The strip re-derives
   *  from the filtered rows, so it morphs as you search, and stays hidden until
   *  you hover the column. */
  columnShapes?: boolean;
}) {
  "use no memo";

  const [sorting, setSorting] = React.useState<SortingState>([]);
  const [globalFilter, setGlobalFilter] = React.useState("");

  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    globalFilterFn: "includesString",
    initialState: {
      pagination: { pageSize: paginationSize },
    },
    state: { sorting, globalFilter },
    onSortingChange: setSorting,
    onGlobalFilterChange: setGlobalFilter,
  });

  // Render strategy: a virtualised scroll by default, classic pagination only
  // when asked for. Either way every row lives on one page — virtual mode just
  // mounts the slice that is in view.
  const paginated = Boolean(pagination);
  React.useEffect(() => {
    table.setPageSize(paginated ? paginationSize : data.length || 1);
  }, [paginated, paginationSize, data.length, table]);

  const term = globalFilter.trim().toLowerCase();
  const rows = table.getRowModel().rows;
  const colCount = table.getVisibleFlatColumns().length;
  const leafColumns = table.getVisibleLeafColumns();

  // ── Fixed-height virtual window ──────────────────────────────────────────
  // Rows are a uniform ROW_HEIGHT, so the visible slice is pure arithmetic on
  // the scroll offset: no per-row measurement, no virtual-list dependency.
  const viewportRef = React.useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = React.useState(0);
  const [viewportH, setViewportH] = React.useState(0);
  const OVERSCAN = 6;
  const startIndex = Math.max(
    0,
    Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN,
  );
  const endIndex = Math.min(
    rows.length,
    Math.ceil((scrollTop + viewportH) / ROW_HEIGHT) + OVERSCAN,
  );
  const windowRows = rows.slice(startIndex, endIndex);
  const padTop = startIndex * ROW_HEIGHT;
  const padBottom = Math.max(0, (rows.length - endIndex) * ROW_HEIGHT);

  // Track the viewport height (and keep it in sync on resize) so the visible
  // slice covers exactly what fits. Re-runs when the pagination mode flips the
  // virtual viewport in or out of the tree.
  React.useEffect(() => {
    const el = viewportRef.current;
    if (paginated || !el) return;
    setViewportH(el.clientHeight);
    const ro = new ResizeObserver(() => setViewportH(el.clientHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, [paginated]);

  // Jump back to the top whenever the filter changes the row set, so the window
  // never lingers past the end of a now-shorter list.
  React.useEffect(() => {
    if (viewportRef.current) viewportRef.current.scrollTop = 0;
    setScrollTop(0);
  }, [globalFilter]);

  // A small chart icon that reveals the column's distribution in a portalled
  // popup on hover. The shape is computed only while the popup is open, and
  // re-derives as the filter narrows the rows, so it morphs live if you search
  // with it open. Portalled so it escapes the scroll viewport's clipping.
  const ColumnDistribution = ({ column }: { column: Column<T, unknown> }) => {
    const [open, setOpen] = React.useState(false);
    const shape = React.useMemo<ColumnShape | undefined>(() => {
      if (!open) return undefined;
      try {
        return computeColumnShape(
          table.getFilteredRowModel().rows.map((r) => r.getValue(column.id)),
        );
      } catch {
        return { kind: "empty" };
      }
      // Re-derive when the popup opens or the filter narrows the rows.
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [open, column.id, globalFilter]);
    return (
      <HoverCard onOpenChange={setOpen}>
        <HoverCardTrigger asChild>
          <button
            type="button"
            aria-label="Show column distribution"
            // Don't let pointer/click on the icon also toggle column sorting.
            onClick={(e) => e.stopPropagation()}
            className="shrink-0 cursor-help rounded text-muted-foreground/40 outline-none transition-colors hover:text-foreground focus-visible:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50"
          >
            <FiBarChart2 className="size-3" />
          </button>
        </HoverCardTrigger>
        <HoverCardContent className="w-56">
          <DistributionView shape={shape} />
        </HoverCardContent>
      </HoverCard>
    );
  };

  // Header cell: a single tight row — label, sort affordance, distribution icon.
  const HeaderCell = ({ header }: { header: Header<T, unknown> }) => {
    const align = (header.column.columnDef.meta as any)?.align;
    const sorted = header.column.getIsSorted();
    const canSort = header.column.getCanSort();
    return (
      <th
        aria-sort={
          sorted === "asc"
            ? "ascending"
            : sorted === "desc"
              ? "descending"
              : undefined
        }
        className={cn(
          "px-2 py-2 text-[12px] font-semibold tracking-wider",
          align === "right" ? "text-right" : "text-left",
          canSort && "cursor-pointer select-none hover:text-foreground",
        )}
        style={{ width: `${header.getSize()}px` }}
        onClick={canSort ? () => header.column.toggleSorting() : undefined}
      >
        <div
          className={cn(
            "flex items-center gap-1",
            align === "right" ? "justify-end" : "justify-start",
          )}
        >
          <span className="truncate">
            {flexRender(header.column.columnDef.header, header.getContext())}
          </span>
          {canSort && (
            <span className="shrink-0 text-[10px] text-muted-foreground">
              {sorted === "asc" ? (
                <FaSortUp size={12} />
              ) : sorted === "desc" ? (
                <FaSortDown size={12} />
              ) : (
                <FaSort size={12} className="opacity-40" />
              )}
            </span>
          )}
          {columnShapes && <ColumnDistribution column={header.column} />}
        </div>
      </th>
    );
  };

  // Body row: shared by both modes. The match highlight is inert when the
  // filter is empty, so the same row works with or without search.
  const BodyRow = ({ row }: { row: Row<T> }) => (
    <tr
      className={cn(
        "h-10 border-b transition-colors hover:bg-muted/50",
        onRowClick && "cursor-pointer",
      )}
      onClick={onRowClick ? () => onRowClick(row.original) : undefined}
      onKeyDown={
        onRowClick
          ? (e) => {
              if (e.key === "Enter") onRowClick(row.original);
            }
          : undefined
      }
      tabIndex={onRowClick ? 0 : undefined}
      role={onRowClick ? "link" : undefined}
    >
      {row.getVisibleCells().map((cell) => {
        const align = (cell.column.columnDef.meta as any)?.align;
        const isMatch =
          term.length > 0 &&
          String(cell.getValue() ?? "")
            .toLowerCase()
            .includes(term);
        return (
          <td
            key={cell.id}
            className={cn(
              "overflow-hidden px-2 text-[14px]",
              align === "right" ? "text-right" : "text-left",
            )}
          >
            <span
              className={cn(
                "block truncate rounded-[4px] px-1 -mx-1",
                isMatch && "bg-primary/[0.08] font-medium",
              )}
            >
              {flexRender(cell.column.columnDef.cell, cell.getContext())}
            </span>
          </td>
        );
      })}
    </tr>
  );

  if (data.length === 0) {
    return <NoDataAvailable />;
  }

  return (
    <div className={cn("flex min-h-0 flex-col", fill && "h-full")}>
      {searchable && (
        <div className="relative mb-3 w-full max-w-sm shrink-0">
          <FiSearch className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground" />
          <input
            type="text"
            value={globalFilter}
            onChange={(e) => setGlobalFilter(e.target.value)}
            placeholder="Filter loaded rows…"
            aria-label="Filter loaded rows"
            className="w-full rounded-md border bg-background py-1.5 pl-8 pr-24 text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/50"
          />
          {term && (
            <div className="absolute right-1.5 top-1/2 flex -translate-y-1/2 items-center gap-1.5">
              <span className="text-[11px] tabular-nums text-muted-foreground">
                {table.getFilteredRowModel().rows.length.toLocaleString()} /{" "}
                {data.length.toLocaleString()}
              </span>
              <button
                type="button"
                onClick={() => setGlobalFilter("")}
                aria-label="Clear filter"
                className="rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
              >
                <FiX className="size-3.5" />
              </button>
            </div>
          )}
        </div>
      )}

      {paginated ? (
        <div className="flex min-h-0 flex-col">
          {/* ── Classic pagination: bounded page, centered controls ── */}
          <div className="overflow-auto rounded-lg border">
            <table className="min-w-full">
              <thead className="sticky top-0 z-10 bg-card">
                {table.getHeaderGroups().map((hg) => (
                  <tr key={hg.id} className="border-b">
                    {hg.headers.map((h) => (
                      <HeaderCell key={h.id} header={h} />
                    ))}
                  </tr>
                ))}
              </thead>
              <tbody>
                {rows.map((row) => (
                  <BodyRow key={row.id} row={row} />
                ))}
              </tbody>
            </table>
          </div>
          <div className="mt-4">
            <TemplatePagination table={table} justifyContent="justify-center" />
          </div>
        </div>
      ) : (
        /* ── Virtualised scroll (default): only the visible slice mounts ── */
        <div
          ref={viewportRef}
          onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
          className={cn(
            "overflow-auto rounded-lg border",
            fill ? "min-h-0 flex-1" : "max-h-[420px]",
          )}
        >
          <table className="min-w-full table-fixed">
            <colgroup>
              {leafColumns.map((col) => (
                <col key={col.id} style={{ width: `${col.getSize()}px` }} />
              ))}
            </colgroup>
            <thead className="sticky top-0 z-10 bg-card">
              {table.getHeaderGroups().map((hg) => (
                <tr key={hg.id} className="border-b">
                  {hg.headers.map((h) => (
                    <HeaderCell key={h.id} header={h} />
                  ))}
                </tr>
              ))}
            </thead>
            <tbody>
              <tr aria-hidden>
                <td className="p-0" colSpan={colCount} style={{ height: `${padTop}px` }} />
              </tr>
              {windowRows.map((row) => (
                <BodyRow key={row.id} row={row} />
              ))}
              <tr aria-hidden>
                <td
                  className="p-0"
                  colSpan={colCount}
                  style={{ height: `${padBottom}px` }}
                />
              </tr>
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
