// dv2_customer_model.cue — A real DV2 model for a customer entity
//
// This is what a data engineer writes. CUE validates it against
// the schema definitions in schema.cue at "compile time" — before
// anything runs.

package wall

// ──────────────────────────────────────────────
// Hub: the unique business entity
// ──────────────────────────────────────────────

hub_customer: #Hub & {
	name:          "hub_customer"
	source_system: "crm_salesforce"
	description:   "Central customer entity sourced from Salesforce CRM"
	business_keys: [{
		name: "bk_customer_id"
		type: "varchar"
	}]
}

// ──────────────────────────────────────────────
// Hub: product entity
// ──────────────────────────────────────────────

hub_product: #Hub & {
	name:          "hub_product"
	source_system: "erp_sap"
	description:   "Product master data from SAP"
	business_keys: [{
		name: "bk_product_sku"
		type: "varchar"
	}, {
		name: "bk_product_region"
		type: "varchar"
	}]
}

// ──────────────────────────────────────────────
// Link: customer buys product
// ──────────────────────────────────────────────

lnk_customer_product: #Link & {
	name:        "lnk_customer_product"
	description: "Tracks which customers purchased which products"
	hub_refs: [{
		hub: "hub_customer"
	}, {
		hub: "hub_product"
	}]
	attributes: [{
		name:        "quantity"
		type:        "integer"
		nullable:    false
		description: "Number of units purchased"
		validations: [{
			check: "range"
			min:   1
			max:   100000
		}]
	}]
}

// ──────────────────────────────────────────────
// Satellite: customer details (changes over time)
// ──────────────────────────────────────────────

sat_customer_details: #Satellite & {
	name:        "sat_customer_details"
	parent:      "hub_customer"
	parent_type: "hub"
	description: "Customer descriptive attributes — tracks history via hash change detection"
	columns: [{
		name:        "email"
		type:        "varchar"
		nullable:    false
		description: "Primary contact email"
		validations: [{
			check: "not_null"
		}, {
			check: "pattern"
			regex: "^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$"
		}]
	}, {
		name:        "company_name"
		type:        "varchar"
		description: "Legal company name"
	}, {
		name:        "tier"
		type:        "varchar"
		nullable:    false
		validations: [{
			check:  "valid_values"
			values: ["free", "starter", "professional", "enterprise"]
		}]
	}, {
		name:     "annual_revenue"
		type:     "decimal"
		nullable: true
		validations: [{
			check: "range"
			min:   0
			max:   1e12
		}]
	}]
}

// ──────────────────────────────────────────────
// Model: materialization config for the hub
// ──────────────────────────────────────────────

model_hub_customer: #Model & {
	name:        "raw_hub_customer"
	description: "Hub customer table in the raw vault"
	materialization: {
		strategy:   "incremental"
		unique_key: "hash_key"
		incremental_on: "load_date"
		dist_style: "key"
		dist_key:   "hash_key"
	}
	depends_on: ["stg_crm_customers"]
	tags: ["dv2", "raw_vault", "customer"]
}
