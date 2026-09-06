//! The OpenAPI document for tako.
//!
//! App-specific (the routes and schemas are tako's); the Swagger UI / spec
//! *mount* is shared — see `service_kit::openapi::swagger_ui`, used in `lib.rs`.

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Tako Ingest API",
        description = "Upload files and load them into the warehouse staging schema",
        version = "0.1.0"
    ),
    tags(
        (name = "Upload", description = "File ingestion into the warehouse"),
        (name = "Table", description = "Staging-table lifecycle and inspection"),
        (name = "Health", description = "Liveness, readiness and host metrics"),
    ),
    paths(
        crate::routes::upload::upload_file,
        crate::routes::table::create_table,
        crate::routes::table::list_tables,
        crate::routes::table::get_table,
        crate::routes::health::health,
        crate::routes::health::readiness,
        crate::routes::health::instance_health,
    ),
    components(schemas(
        crate::error::ErrorResponse,
        crate::routes::table::TableList,
        crate::routes::table::TableDetail,
        crate::routes::table::ColumnView,
        crate::routes::health::InstanceHealth,
        crate::routes::health::CpuInfo,
        crate::routes::health::MemoryInfo,
    ))
)]
struct ApiDoc;

/// The assembled OpenAPI document, mounted in `create_router`.
pub fn api_doc() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_assembles_and_lists_every_route() {
        let json = api_doc().to_json().expect("OpenAPI doc serializes to JSON");
        for path in [
            "/upload",
            "/table",
            "/table/{name}",
            "/health",
            "/health/ready",
            "/instance_health",
        ] {
            assert!(
                json.contains(&format!("\"{path}\"")),
                "spec is missing {path}"
            );
        }
        // The shared error schema must be present too.
        assert!(
            json.contains("ErrorResponse"),
            "spec is missing ErrorResponse"
        );
    }
}
