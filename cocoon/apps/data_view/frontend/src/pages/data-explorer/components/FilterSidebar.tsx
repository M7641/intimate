import React from "react";
import { Filter, Plus, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import ColumnFilterCard from "./ColumnFilter";

import type { ColumnFilter, ColumnMeta } from "@/pages/_shared/types";

interface FilterSidebarProps {
  columns: ColumnMeta[];
  filters: ColumnFilter[];
  onFiltersChange: (filters: ColumnFilter[]) => void;
  disabled?: boolean;
  selectedSchema: string;
  selectedTable: string;
  selectedTimestamp?: string;
}

function generateFilterId(): string {
  return `filter-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}

export default function FilterSidebar({
  columns,
  filters,
  onFiltersChange,
  disabled = false,
  selectedSchema,
  selectedTable,
  selectedTimestamp,
}: FilterSidebarProps) {
  const scrollRef = React.useRef<HTMLDivElement>(null);

  const usedColumns = filters.map((f) => f.column).filter(Boolean);
  const activeFilterCount = filters.filter((f) => f.column && f.value).length;

  const handleAddFilter = () => {
    const newFilter: ColumnFilter = {
      id: generateFilterId(),
      column: "",
      operator: "equals",
      value: "",
    };
    onFiltersChange([...filters, newFilter]);

    // Auto-scroll to show the new filter card
    requestAnimationFrame(() => {
      if (scrollRef.current) {
        scrollRef.current.scrollTo({
          top: scrollRef.current.scrollHeight,
          behavior: "smooth",
        });
      }
    });
  };

  const handleUpdateFilter = (id: string, updatedFilter: ColumnFilter) => {
    onFiltersChange(filters.map((f) => (f.id === id ? updatedFilter : f)));
  };

  const handleRemoveFilter = (id: string) => {
    onFiltersChange(filters.filter((f) => f.id !== id));
  };

  const handleClearAll = () => {
    onFiltersChange([]);
  };

  return (
    <aside className="w-72 shrink-0 bg-sidebar text-sidebar-foreground border-r border-sidebar-border flex flex-col">
      {/* Header */}
      <div className="flex items-center gap-2 px-4 py-3 border-b border-sidebar-border">
        <Filter className="h-4 w-4 text-sidebar-foreground/70" />
        <span className="text-sm font-semibold">Filters</span>
        {activeFilterCount > 0 && (
          <span className="rounded-full bg-primary text-primary-foreground px-2 py-0.5 text-xs leading-none">
            {activeFilterCount}
          </span>
        )}
        {filters.length > 0 && (
          <Button
            variant="ghost"
            size="sm"
            onClick={handleClearAll}
            className="ml-auto h-7 text-xs text-sidebar-foreground/70 hover:text-sidebar-foreground"
          >
            <X className="h-3 w-3 mr-1" />
            Clear
          </Button>
        )}
      </div>

      {/* Scrollable body */}
      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto px-3 py-3 space-y-2.5"
      >
        {filters.length === 0 ? (
          <p className="text-xs text-sidebar-foreground/50 text-center py-6">
            No filters applied.
          </p>
        ) : (
          filters.map((filter) => (
            <ColumnFilterCard
              key={filter.id}
              filter={filter}
              columns={columns}
              usedColumns={usedColumns}
              selectedSchema={selectedSchema}
              selectedTable={selectedTable}
              selectedTimestamp={selectedTimestamp}
              onUpdate={(updated) => handleUpdateFilter(filter.id, updated)}
              onRemove={() => handleRemoveFilter(filter.id)}
            />
          ))
        )}
      </div>

      {/* Footer — sticky add button */}
      <div className="px-3 py-3 border-t border-sidebar-border">
        <Button
          variant="outline"
          size="sm"
          onClick={handleAddFilter}
          disabled={disabled || columns.length === 0}
          className="w-full h-8 text-xs bg-sidebar hover:bg-sidebar-accent"
        >
          <Plus className="h-3.5 w-3.5 mr-1.5" />
          Add Filter
        </Button>
      </div>
    </aside>
  );
}
