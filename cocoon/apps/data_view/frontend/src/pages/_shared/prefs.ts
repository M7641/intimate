// User preferences persisted across visits via localStorage.

// The schema the user is working in. Shared across Catalogue, Table Info and
// Column Analysis so a choice on one page is remembered everywhere. A deep
// link (?schema=) always takes precedence over the stored value.
export const SCHEMA_STORAGE_KEY = "data-view-schema";
