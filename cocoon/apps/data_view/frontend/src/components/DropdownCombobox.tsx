import * as React from "react";
import { Check, ChevronsUpDown } from "lucide-react";

import { cn } from "@/lib/utils";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";

interface Option {
  value: string;
  label: string;
}

interface DropdownComboboxProps {
  placeholder?: string;
  label?: string;
  options: Option[];
  empty_text?: string;
  value?: string;
  clearable?: boolean;
  onValueChange?: (value: string) => void;
  width?: string;
  className?: string;
  disabled?: boolean;
}

/**
 * Searchable single-select. Keeps the same external API as before (string
 * `value` + `onValueChange`) so callers are unchanged. Built on the shadcn
 * Popover plus a typed filter over the options, so it stays searchable without
 * pulling in a command-palette dependency.
 */
export default function DropdownCombobox({
  placeholder = "Select...",
  label = "Select...",
  options,
  empty_text = "No options found.",
  value,
  onValueChange,
  width = "w-full",
  className,
  disabled = false,
}: DropdownComboboxProps) {
  const [open, setOpen] = React.useState(false);
  const [query, setQuery] = React.useState("");
  const inputRef = React.useRef<HTMLInputElement | null>(null);

  const selected = options.find((o) => o.value === value) ?? null;

  // Reset the typed filter whenever the popup closes, so it reopens clean.
  React.useEffect(() => {
    if (!open) setQuery("");
  }, [open]);

  // Focus the search field as soon as the popup opens.
  React.useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  const term = query.trim().toLowerCase();
  const filtered = term
    ? options.filter((o) => o.label.toLowerCase().includes(term))
    : options;

  const select = (next: string) => {
    onValueChange?.(next);
    setOpen(false);
  };

  return (
    <Popover open={open} onOpenChange={disabled ? undefined : setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          role="combobox"
          aria-expanded={open}
          disabled={disabled}
          className={cn(
            "border-input flex h-9 items-center justify-between gap-2 rounded-md border bg-transparent px-3 text-sm shadow-xs outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-input/30",
            width,
            className,
          )}
        >
          {selected ? (
            <span className="truncate">{selected.label}</span>
          ) : (
            <span className="truncate text-muted-foreground">
              {placeholder}
            </span>
          )}
          <ChevronsUpDown className="size-4 shrink-0 opacity-50" />
        </button>
      </PopoverTrigger>
      <PopoverContent className={cn(width, "p-0")} align="start">
        <div className="flex items-center border-b px-3">
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={label}
            className="h-9 w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground"
          />
        </div>
        <div className="max-h-72 overflow-y-auto p-1">
          {filtered.length === 0 ? (
            <p className="py-6 text-center text-sm text-muted-foreground">
              {empty_text}
            </p>
          ) : (
            filtered.map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => select(option.value)}
                className="relative flex w-full cursor-default items-center justify-between gap-2 rounded-sm px-2 py-1.5 text-sm outline-hidden select-none hover:bg-accent hover:text-accent-foreground"
              >
                <span className="truncate">{option.label}</span>
                <Check
                  className={cn(
                    "size-4 shrink-0",
                    value === option.value ? "opacity-100" : "opacity-0",
                  )}
                />
              </button>
            ))
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}
