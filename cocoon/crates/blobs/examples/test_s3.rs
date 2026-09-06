use blobs::{StorageBackend, new};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Bucket: {}", std::env::var("DATA_LAKE").unwrap());

    // Create S3 client using the new factory pattern
    let client = new(StorageBackend::s3(std::env::var("DATA_LAKE").unwrap())).await?;

    println!("Backend: {}", client.backend_name());

    // List all files (no prefix filter)
    let files = client.list_files(None).await?;

    for file in files {
        println!("{}", file);
    }

    Ok(())
}
