import type { ColumnDef } from "@tanstack/react-table";
import MTable from "@/components/MTable";
import NoDataAvailable from "@/components/NoDataAvailable";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

export interface FilterConfig {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: Array<{ value: string; label: string }>;
  width?: string;
}

interface FilteredTablePageProps<T> {
  title: string;
  description: string;
  filters: FilterConfig[];
  columns: ColumnDef<T, any>[];
  data: T[];
  loading: boolean;
  renderStats?: () => React.ReactNode;
  pageSize?: number;
  emptyMessage?: string;
}

export default function FilteredTablePage<T>({
  title,
  description,
  filters,
  columns,
  data,
  loading,
  renderStats,
  pageSize = 20,
  emptyMessage = "No data found",
}: FilteredTablePageProps<T>) {
  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h1 className="text-2xl font-bold">{title}</h1>
          <p className="text-muted-foreground">{description}</p>
        </div>
        <div className="flex flex-wrap items-center gap-4">
          {!loading && renderStats?.()}
          {filters.map((filter) => (
            <div key={filter.label} className="flex items-center gap-2">
              <span className="text-sm text-muted-foreground">
                {filter.label}
              </span>
              <Select value={filter.value} onValueChange={filter.onChange}>
                <SelectTrigger
                  aria-label={filter.label}
                  className={filter.width ?? "w-32"}
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {filter.options.map((opt) => (
                    <SelectItem key={opt.value} value={opt.value}>
                      {opt.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          ))}
        </div>
      </div>

      <div className="bg-card rounded-lg border p-4">
        {loading ? (
          <div className="h-96 animate-pulse rounded bg-muted" />
        ) : data.length === 0 ? (
          <NoDataAvailable message={emptyMessage} description="Try widening the time range or adjusting filters." />
        ) : (
          <MTable
            data={data}
            columns={columns}
            pagination={true}
            paginationSize={pageSize}
          />
        )}
      </div>
    </div>
  );
}
