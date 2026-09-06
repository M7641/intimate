import { useEffect, useState } from "react";

/// Returns `value` delayed by `delayMs`, resetting the timer on each change —
/// so rapidly-changing inputs (e.g. a search box) only settle once idle.
export function useDebouncedValue<T>(value: T, delayMs = 300): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), delayMs);
    return () => clearTimeout(id);
  }, [value, delayMs]);
  return debounced;
}
