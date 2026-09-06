import { X } from "lucide-react";

import DropdownCombobox from "@/components/DropdownCombobox";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";

import type {
  ColumnFilter,
  ColumnMeta,
  FilterOperator,
} from "@/pages/_shared/types";
import { useDataExplorerColumnValues } from "@/pages/_shared/api";

interface ColumnFilterProps {
  filter: ColumnFilter;
  columns: ColumnMeta[];
  usedColumns: string[];
  selectedSchema: string;
  selectedTable: string;
  selectedTimestamp?: string;
  onUpdate: (filter: ColumnFilter) => void;
  onRemove: () => void;
}

const OPERATORS: { value: FilterOperator; label: string }[] = [
  { value: "equals", label: "equals" },
  { value: "contains", label: "contains" },
];

const CARDINALITY_THRESHOLD = 50;

export default function ColumnFilterCard({
  filter,
  columns,
  usedColumns,
  selectedSchema,
  selectedTable,
  selectedTimestamp,
  onUpdate,
  onRemove,
}: ColumnFilterProps) {
  const { data: columnValues, isLoading: valuesLoading } =
    useDataExplorerColumnValues(
      selectedSchema,
      selectedTable,
      filter.column,
      selectedTimestamp,
    );

  const availableColumns = columns.filter(
    (c) => c.key === filter.column || !usedColumns.includes(c.key),
  );

  const useDropdownForValue =
    columnValues && columnValues.length <= CARDINALITY_THRESHOLD;

  return (
    <div className="space-y-2 rounded-lg border border-sidebar-border bg-sidebar-accent/30 p-2.5">
      {/* Row 1: Column selector + remove button */}
      <div className="flex items-center gap-1.5">
        <DropdownCombobox
          placeholder="Select column..."
          options={availableColumns.map((c) => ({
            value: c.key,
            label: c.displayName,
          }))}
          value={filter.column}
          onValueChange={(value) =>
            onUpdate({ ...filter, column: value, value: "" })
          }
          className="h-8 text-xs flex-1"
        />
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onRemove}
          className="shrink-0 text-muted-foreground hover:text-destructive"
        >
          <X className="h-3.5 w-3.5" />
        </Button>
      </div>

      {/* Row 2: Operator */}
      <Select
        value={filter.operator}
        onValueChange={(value) =>
          onUpdate({ ...filter, operator: value as FilterOperator })
        }
        disabled={!filter.column}
      >
        <SelectTrigger className="h-8 w-full text-xs">
          <SelectValue placeholder="Operator" />
        </SelectTrigger>
        <SelectContent>
          {OPERATORS.map((op) => (
            <SelectItem key={op.value} value={op.value}>
              {op.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      {/* Row 3: Value input */}
      {valuesLoading ? (
        <Skeleton className="h-8 w-full rounded-md" />
      ) : useDropdownForValue ? (
        <DropdownCombobox
          placeholder="Select value..."
          options={(columnValues ?? []).map((v) => ({
            value: v,
            label: v || "(empty)",
          }))}
          value={filter.value}
          onValueChange={(value) => onUpdate({ ...filter, value })}
          className="h-8 w-full text-xs"
          disabled={!filter.column}
        />
      ) : (
        <Input
          placeholder="Enter value..."
          value={filter.value}
          onChange={(e) => onUpdate({ ...filter, value: e.target.value })}
          className="h-8 w-full text-xs"
          disabled={!filter.column}
        />
      )}
    </div>
  );
}
