// invalid_examples.cue — Intentionally broken configurations
//
// Uncomment each block one at a time and run `cue vet ./...`
// to see CUE catch the errors at "compile time".
//
// This file demonstrates the value proposition: errors that would
// currently be runtime surprises become definition-time rejections.

package wall

// ──────────────────────────────────────────────
// Example 1: Hub with no business keys
// ERROR: business_keys must have at least one element
// ──────────────────────────────────────────────

// bad_hub_no_keys: #Hub & {
// 	name:          "hub_empty"
// 	source_system: "test"
// 	business_keys: []  // ← CUE rejects: list needs at least 1 element
// }

// ──────────────────────────────────────────────
// Example 2: Link referencing only one hub
// ERROR: hub_refs must have at least two elements
// ──────────────────────────────────────────────

// bad_link_one_hub: #Link & {
// 	name: "lnk_lonely"
// 	hub_refs: [{hub: "hub_customer"}]  // ← needs >= 2 hubs
// }

// ──────────────────────────────────────────────
// Example 3: Validation range where min > max
// ERROR: max must be greater than min
// ──────────────────────────────────────────────

// bad_range: #ValidationRule & {
// 	check: "range"
// 	min:   100
// 	max:   50  // ← CUE rejects: max (50) is not > min (100)
// }

// ──────────────────────────────────────────────
// Example 4: Hub name doesn't match pattern
// ERROR: name must match ^hub_[a-z_]+$
// ──────────────────────────────────────────────

// bad_hub_name: #Hub & {
// 	name:          "Hub_Customer"  // ← uppercase not allowed
// 	source_system: "test"
// 	business_keys: [{name: "bk_id", type: "varchar"}]
// }

// ──────────────────────────────────────────────
// Example 5: S3 source missing required bucket
// ERROR: bucket is required when type is "s3"
// ──────────────────────────────────────────────

// bad_source: #Source & {
// 	type:   "s3"
// 	format: "parquet"
// 	// bucket and prefix missing → CUE rejects
// }

// ──────────────────────────────────────────────
// Example 6: Pipeline with invalid version format
// ERROR: version must match semver pattern
// ──────────────────────────────────────────────

// bad_version: #Pipeline & {
// 	name:    "my-pipeline"
// 	version: "v1"  // ← must be "X.Y.Z" format
// 	source: {type: "s3", format: "parquet", bucket: "b", prefix: "p"}
// 	output: {type: "s3", bucket: "b", prefix: "p"}
// 	schedule: {type: "cron", expression: "0 0 * * *"}
// }

// ──────────────────────────────────────────────
// Example 7: Extra field on a closed struct
// ERROR: #Hub is closed — no extra fields allowed
// ──────────────────────────────────────────────

// bad_extra_field: #Hub & {
// 	name:          "hub_test"
// 	source_system: "test"
// 	business_keys: [{name: "bk_id", type: "varchar"}]
// 	color:         "blue"  // ← field not in #Hub definition
// }
