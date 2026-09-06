// pipeline_s3_to_redshift.cue — Example customer pipeline configuration
//
// A typical ingestion pipeline: S3 parquet → quality checks → Redshift.
// CUE validates this entire configuration before the pipeline executes.

package wall

// ──────────────────────────────────────────────
// A production pipeline from S3 to Redshift
// ──────────────────────────────────────────────

pipeline_customer_ingest: #Pipeline & {
	name:        "customer-daily-ingest"
	version:     "1.2.0"
	description: "Daily ingestion of customer events from S3 landing zone to Redshift raw vault"

	source: {
		type:   "s3"
		format: "parquet"
		bucket: "acme-data-lake-prod"
		prefix: "landing/crm/customers/"
		region: "eu-west-1"
	}

	validations: [{
		name:     "customer_id_present"
		field:    "customer_id"
		check:    "not_null"
		severity: "error"
	}, {
		name:     "email_format"
		field:    "email"
		check:    "pattern"
		regex:    "^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$"
		severity: "warning" // warn but don't block
	}, {
		name:           "data_freshness"
		field:          "event_timestamp"
		check:          "freshness"
		max_age_hours:  48
		severity:       "error"
	}, {
		name:     "minimum_rows"
		field:    "*"
		check:    "row_count"
		min_rows: 100
		max_rows: 10000000
		severity: "error"
	}]

	transforms: [{
		name:  "deduplicate_and_hash"
		type:  "sql"
		query: """
			SELECT DISTINCT
				MD5(customer_id) AS hash_key,
				customer_id,
				email,
				company_name,
				tier,
				CURRENT_TIMESTAMP AS load_date
			FROM source_data
			"""
	}]

	output: {
		type:   "redshift"
		schema: "raw_vault"
		table:  "hub_customer"
		mode:   "merge"
		merge_key: "hash_key"
	}

	schedule: {
		type:       "cron"
		expression: "30 6 * * *" // 6:30 AM daily
		timezone:   "Europe/London"
	}

	alerts: {
		channels: [{
			type:    "slack"
			webhook: "https://hooks.slack.com/services/T00/B00/xxxxx"
		}, {
			type:       "email"
			recipients: ["data-team@acme.com", "oncall@acme.com"]
		}]
		on_failure:         true
		on_quality_warning: true
		on_sla_breach:      true
	}
}
