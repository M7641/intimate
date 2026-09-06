//! Shared OpenAPI mounting for the cocoon axum services.
//!
//! Each app owns its own `#[derive(OpenApi)] ApiDoc` (the routes and schemas are
//! app-specific), but the *mount points* should be identical everywhere. This
//! centralises them so every service serves its spec and UI at the same paths.

use utoipa::openapi::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

/// Standard Swagger UI + raw-spec mount for an app's OpenAPI document.
///
/// Serves the interactive UI at `/swagger-ui` and the raw spec at
/// `/api-docs/openapi.json`. Merge it into the router:
///
/// ```ignore
/// let router = Router::new()
///     .route(/* … */)
///     .merge(service_kit::openapi::swagger_ui(ApiDoc::openapi()));
/// ```
pub fn swagger_ui(doc: OpenApi) -> SwaggerUi {
    SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", doc)
}
