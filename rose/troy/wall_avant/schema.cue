// schema.cue — Data Vault 2.0 schema definitions in CUE
//
// This is the "wall" — everything outside is untrusted text,
// everything inside is validated, typed data.

package wall

import "strings"

// ──────────────────────────────────────────────
// Core DV2 entity definitions
// ──────────────────────────────────────────────

// A Hub captures a unique business key.
#Hub: {
	name!:         string & =~"^hub_[a-z_]+$"
	business_keys: [...#BusinessKey] & [_, ...] // at least one
	description?:  string
	source_system: string
	load_date_column: *"load_date" | string
}

#BusinessKey: {
	name!: string & =~"^bk_[a-z_]+$"
	type!: #ColumnType
}

// A Link captures a relationship between two or more Hubs.
#Link: {
	name!:        string & =~"^lnk_[a-z_]+$"
	hub_refs:     [...#HubRef] & [_, _, ...] // at least two hubs
	description?: string
	// Optional degenerate attributes on the link itself
	attributes?: [...#Column]
}

#HubRef: {
	hub!:  string & =~"^hub_[a-z_]+$"
	role?: string // e.g. "parent", "child" for same-as links
}

// A Satellite holds descriptive/contextual attributes for a Hub or Link.
#Satellite: {
	name!:       string & =~"^sat_[a-z_]+$"
	parent!:     string & (=~"^hub_[a-z_]+$" | =~"^lnk_[a-z_]+$")
	parent_type: *"hub" | "link"
	columns:     [...#Column] & [_, ...] // at least one column
	description?: string
	change_detection: *"hash" | "full_diff"
}

// ──────────────────────────────────────────────
// Column types and validation rules
// ──────────────────────────────────────────────

#ColumnType: "varchar" | "integer" | "bigint" | "decimal" | "boolean" | "timestamp" | "date" | "json"

#Column: {
	name!:        string & =~"^[a-z][a-z0-9_]*$"
	type!:        #ColumnType
	nullable:     *true | bool
	description?: string
	validations?: [...#ValidationRule]
}

#ValidationRule: {
	check!: "not_null" | "unique" | "range" | "pattern" | "valid_values"
	if check == "range" {
		min!: number
		max!: number & >min
	}
	if check == "pattern" {
		regex!: string
	}
	if check == "valid_values" {
		values!: [...string] & [_, ...]
	}
}

// ──────────────────────────────────────────────
// Warehouse materialization config
// ──────────────────────────────────────────────

#MaterializationConfig: {
	strategy!: "table" | "view" | "incremental"
	if strategy == "incremental" {
		unique_key!:     string
		incremental_on!: string
	}
	dist_style?: "key" | "even" | "all"
	if dist_style == "key" {
		dist_key!: string
	}
	sort_key?: string
	schema:    *"analytics" | string
}

// ──────────────────────────────────────────────
// Full model definition — ties it all together
// ──────────────────────────────────────────────

#Model: {
	name!:            string & strings.MinRunes(3)
	description?:     string
	materialization!: #MaterializationConfig
	depends_on?:      [...string]
	tags?:            [...string]
}
