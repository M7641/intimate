```rust
/// Custom key extractor that falls back to a default IP when extraction fails
#[derive(Clone, Copy, Debug)]
pub struct FallbackIpKeyExtractor;

impl KeyExtractor for FallbackIpKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(
        &self,
        req: &axum::http::Request<T>,
    ) -> Result<Self::Key, tower_governor::GovernorError> {
        // Try to extract from x-forwarded-for header first
        if let Some(forwarded) = req.headers().get("x-forwarded-for") {
            if let Ok(forwarded_str) = forwarded.to_str() {
                if let Some(ip_str) = forwarded_str.split(',').next() {
                    if let Ok(ip) = ip_str.trim().parse::<IpAddr>() {
                        return Ok(ip);
                    }
                }
            }
        }

        // Try to extract from x-real-ip header
        if let Some(real_ip) = req.headers().get("x-real-ip") {
            if let Ok(ip_str) = real_ip.to_str() {
                if let Ok(ip) = ip_str.parse::<IpAddr>() {
                    return Ok(ip);
                }
            }
        }

        // Fall back to localhost for local development
        Ok(IpAddr::from([127, 0, 0, 1]))
    }
}

// Configure rate limiting: 100 request per second per IP
// Uses custom key extractor that falls back to localhost for local development
// https://docs.rs/tower_governor/latest/tower_governor/
let governor_conf = GovernorConfigBuilder::default()
    .key_extractor(FallbackIpKeyExtractor)
    .per_second(10000)
    .burst_size(100)
    .finish()
    .unwrap();
```
