// pipeline.cue — Data pipeline configuration schema
//
// Defines the shape of a pipeline: source → validations → transforms → output.
// Customers write their pipeline configs against these definitions.

package wall

// ──────────────────────────────────────────────
// Pipeline definition
// ──────────────────────────────────────────────

#Pipeline: {
	name!:        string & =~"^[a-z][a-z0-9_-]+$"
	version!:     string & =~"^[0-9]+\\.[0-9]+\\.[0-9]+$"
	description?: string
	source!:      #Source
	validations?: [...#QualityRule]
	transforms?:  [...#Transform]
	output!:      #Output
	schedule!:    #Schedule
	alerts?:      #AlertConfig
}

// ──────────────────────────────────────────────
// Source — where the data comes from
// ──────────────────────────────────────────────

#Source: {
	type!:   "s3" | "postgres" | "api" | "kafka"
	format!: "parquet" | "csv" | "json" | "avro"

	if type == "s3" {
		bucket!: string
		prefix!: string
		region:  *"eu-west-1" | string
	}
	if type == "postgres" {
		host!:   string
		port:    *5432 | int & >0 & <65536
		dbname!: string
		schema:  *"public" | string
		table!:  string
	}
	if type == "api" {
		url!:          string & =~"^https://"
		auth_method!:  "bearer" | "api_key" | "oauth2"
		rate_limit?:   int & >0
	}
	if type == "kafka" {
		brokers!:       [...string] & [_, ...]
		topic!:         string
		consumer_group: *"default" | string
	}
}

// ──────────────────────────────────────────────
// Data quality rules
// ──────────────────────────────────────────────

#QualityRule: {
	name!:   string
	field!:  string
	check!:  "not_null" | "unique" | "range" | "pattern" | "freshness" | "row_count"
	severity: *"error" | "warning"

	if check == "range" {
		min!: number
		max!: number & >min
	}
	if check == "pattern" {
		regex!: string
	}
	if check == "freshness" {
		max_age_hours!: int & >0
	}
	if check == "row_count" {
		min_rows!: int & >=0
		max_rows?: int & >min_rows
	}
}

// ──────────────────────────────────────────────
// Transforms
// ──────────────────────────────────────────────

#Transform: {
	name!: string
	type!: "sql" | "dbt_model" | "python" | "arrow"
	if type == "sql" {
		query!: string
	}
	if type == "dbt_model" {
		model_name!: string
	}
	if type == "python" {
		module!:   string
		function!: string
	}
	if type == "arrow" {
		operations!: [...("filter" | "project" | "aggregate" | "join")]
		expression!: string
	}
}

// ──────────────────────────────────────────────
// Output
// ──────────────────────────────────────────────

#Output: {
	type!:   "s3" | "redshift" | "snowflake" | "postgres"
	format:  *"parquet" | "csv" | "json"

	if type == "s3" {
		bucket!:     string
		prefix!:     string
		partitions?: [...string]
	}
	if type == "redshift" || type == "snowflake" || type == "postgres" {
		schema!: string
		table!:  string
		mode!:   "append" | "replace" | "merge"
		if mode == "merge" {
			merge_key!: string
		}
	}
}

// ──────────────────────────────────────────────
// Schedule
// ──────────────────────────────────────────────

#Schedule: {
	type!: "cron" | "interval" | "event"
	if type == "cron" {
		expression!: string & =~"^[0-9*,/-]+ [0-9*,/-]+ [0-9*,/-]+ [0-9*,/-]+ [0-9*,/-]+$"
	}
	if type == "interval" {
		every!: string & =~"^[0-9]+(m|h|d)$"
	}
	if type == "event" {
		trigger!: string
	}
	timezone: *"UTC" | string
}

// ──────────────────────────────────────────────
// Alerts
// ──────────────────────────────────────────────

#AlertConfig: {
	channels!: [...#AlertChannel] & [_, ...]
	on_failure:          *true | bool
	on_quality_warning:  *false | bool
	on_sla_breach:       *false | bool
}

#AlertChannel: {
	type!: "slack" | "email" | "pagerduty"
	if type == "slack" {
		webhook!: string & =~"^https://hooks\\.slack\\.com/"
	}
	if type == "email" {
		recipients!: [...string & =~"@"] & [_, ...]
	}
	if type == "pagerduty" {
		service_key!: string
	}
}
