# Configuration Languages — CUE, Pkl, and the Case Against YAML

## The Load-Bearing Surface

The bespoke-to-product transition moves logic from code to configuration. Schema definitions, validation rules, pipeline parameters, customer settings, deployment manifests — all expressed as configuration. This configuration is now the primary interface between the product and its users. It's load-bearing. And YAML is not up to the task.

YAML's problems are well-documented: no type system, no schema validation (without external tools), no way to compose or abstract, implicit type coercion ("yes" becomes a boolean, "1.0" becomes a float), indentation-sensitive syntax that causes silent errors. JSON is worse (no comments, more verbose). TOML is better for flat configuration but can't express nested or relational structures well.

When configuration replaces code, the configuration language needs properties that code has: types, validation, composition, and abstraction. Three languages offer this.

## CUE

Created by Marcel van Lohuizen (who worked on Go, Borg, and the Kubernetes configuration system at Google). CUE treats configuration as _constrained data_ — you define types and constraints, and CUE verifies that values satisfy them.

The core idea: in CUE, types and values exist on the same lattice. A type is just a set of constraints. A value is a maximally constrained type. This means you can progressively narrow configuration — start with a broad type, add constraints at each layer, and CUE guarantees the result is consistent.

```cue
// Schema definition
#Pipeline: {
    source: #Source
    validations: [...#Rule]
    output: #OutputFormat
    schedule: string & =~"^[0-9]+ [0-9]+ \\* \\* [0-9,\\-\\*]+$"  // cron pattern
}

#Source: {
    type: "s3" | "postgres" | "api"
    path: string
    format: "parquet" | "csv" | "json"
    if type == "s3" {
        bucket: string
        region: string
    }
}

#Rule: {
    field: string
    check: "not_null" | "unique" | "range" | "pattern"
    if check == "range" {
        min: number
        max: number & >min
    }
}
```

This is parsing, not validation. The schema defines what legal configuration looks like. If a customer's configuration doesn't satisfy the constraints, CUE rejects it with a clear error — before anything runs.

**Why CUE specifically:** It has built-in support for generating configuration from schemas (scaffolding), validating existing configuration against schemas, and merging partial configurations (layering defaults with overrides). It's also designed for large-scale configuration — Google built it because Kubernetes YAML was unmanageable at their scale.

**CUE for DV2 schemas:** The DV2 schema definitions (hubs, links, satellites, business keys) are currently described in prose. In CUE, they'd be typed configuration that validates at definition time. "This satellite references a hub that doesn't exist" becomes a compile-time error, not a runtime surprise.

## Pkl

Created by Apple. Pkl (pronounced "pickle") is a configuration language with a more familiar syntax — closer to TypeScript or Kotlin than CUE's lattice-based model. It supports classes, inheritance, type annotations, and computed properties.

```pkl
class Pipeline {
    source: Source
    validations: Listing<Rule>
    output: OutputFormat
    schedule: String(matches(Regex("^[0-9]+ [0-9]+ \\* \\* [0-9,\\-\\*]+$")))
}

class Source {
    type: "s3" | "postgres" | "api"
    path: String
    format: "parquet" | "csv" | "json"
}
```

Pkl's advantages over CUE: more intuitive syntax for developers coming from mainstream languages, better IDE support, and a module system that feels like importing code. Its disadvantage: less powerful constraint system (CUE's lattice model handles complex intersections that Pkl's type system can't express as naturally).

**Pkl for customer configuration:** If customers write their own pipeline configurations, Pkl's familiar syntax reduces adoption friction. The class-based model is easier to teach than CUE's constraint lattice.

## Dhall

Worth mentioning for completeness. Dhall is a programmable configuration language that's guaranteed to terminate (no infinite loops) and guaranteed to be side-effect-free (no I/O). It's the most principled option — a total functional language for configuration. But it's also the most alien syntax and the smallest ecosystem. Consider it if mathematical purity appeals; ignore it otherwise.

## The Configuration Architecture

Regardless of which language, the architecture pattern is:

```
Configuration Schema (CUE/Pkl)
    ↓ validates
Customer Configuration Files
    ↓ generates
Internal Runtime Configuration (Rust structs, Python dataclasses)
    ↓ drives
Pipeline Execution
```

The schema is the contract. Customer configuration is validated against it. The validated configuration is code-generated into language-native types (CUE exports to JSON/YAML/Protobuf; Pkl exports to JSON/YAML/Java/Kotlin/Swift). The pipeline code works with typed, validated configuration — no parsing needed at runtime because the configuration language already parsed it.

This is the wall strategy applied to configuration. The wall is the schema. Everything outside the wall is untrusted text. Everything inside is validated, typed data.

## Where to Start

Take the messiest configuration in the current system — probably the DV2 schema definitions or the customer pipeline parameters — and express it in CUE. Validate one real customer's configuration against it. Count the errors CUE catches that the current system would have let through. That count is the argument for adoption.

CUE over Pkl for the initial evaluation: CUE's constraint system is more naturally suited to data validation (the primary use case), and its Kubernetes heritage means it handles the kind of layered, overridable configuration that multi-tenant products need.
