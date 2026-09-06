// layered_defaults.cue — Demonstrates CUE's approach to defaults + overrides
//
// CUE doesn't do "deep merge with override" — it does *unification*.
// Two concrete values that differ is a conflict (by design — catches bugs).
//
// Instead, defaults use disjunctions (*default | type), and the specific
// layer picks the value. Only one layer provides the concrete value.

package wall

// ──────────────────────────────────────────────
// Approach 1: Default values via disjunctions
// The schema provides defaults; the instance picks specific values.
// ──────────────────────────────────────────────

#PipelineConfig: {
	schedule: {
		expression: *"0 2 * * *" | string  // default: 2 AM
		timezone:   *"UTC" | string         // default: UTC
	}
	output: {
		format: *"parquet" | "csv" | "json"
	}
	alerts: {
		on_failure:         *true | bool
		on_quality_warning: *false | bool
		on_sla_breach:      *false | bool
	}
}

// Customer ACME overrides only what they need.
// Everything else gets the defaults from #PipelineConfig.
acme_config: #PipelineConfig & {
	schedule: {
		expression: "30 6 * * *"      // ACME wants 6:30 AM
		timezone:   "Europe/London"   // ACME is UK-based
	}
	alerts: {
		on_quality_warning: true      // ACME wants quality alerts
		on_sla_breach:      true      // and SLA breach alerts
		// on_failure stays true (default)
	}
	// output.format stays "parquet" (default)
}

// Customer Beta uses all defaults — just validates against the schema.
beta_config: #PipelineConfig & {}

// ──────────────────────────────────────────────
// Approach 2: Template pattern for shared structure
// Define a template, then stamp out instances.
// ──────────────────────────────────────────────

#TenantDeployment: {
	tenant_id!:  string
	environment: "prod" | "staging" | "dev"
	region:      "eu-west-1" | "us-east-1" | "ap-southeast-1"
	pipeline:    #PipelineConfig
	resources: {
		max_memory_gb: int & >=1 & <=64
		max_cpu_cores: int & >=1 & <=16
		max_concurrent_pipelines: int & >=1 & <=10
	}
}

// Stamp out tenant deployments
deployments: [Name=string]: #TenantDeployment & {tenant_id: Name}

deployments: {
	"acme-corp": {
		environment: "prod"
		region:      "eu-west-1"
		pipeline:    acme_config
		resources: {
			max_memory_gb: 32
			max_cpu_cores: 8
			max_concurrent_pipelines: 5
		}
	}
	"beta-startup": {
		environment: "staging"
		region:      "us-east-1"
		pipeline:    beta_config
		resources: {
			max_memory_gb: 4
			max_cpu_cores: 2
			max_concurrent_pipelines: 1
		}
	}
}
