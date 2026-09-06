use blobs::{StorageBackend, new};
use bytes::Bytes;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create local filesystem client
    let root_path = std::env::var("BLOB_ROOT").unwrap_or_else(|_| "./tmp".to_string());
    println!("Root path: {}", root_path);

    let client = new(StorageBackend::local(root_path)).await?;
    println!("Backend: {}", client.backend_name());

    // Upload a test file
    let test_data = Bytes::from("Hello, local storage!");
    client.upload_object("test/hello.txt", test_data).await?;
    println!("Uploaded: test/hello.txt");

    // Check if file exists
    let exists = client.file_exists("test/hello.txt").await?;
    println!("File exists: {}", exists);

    // Download and print the file
    let data = client.download_file("test/hello.txt").await?;
    println!("Content: {}", String::from_utf8_lossy(&data));

    // Get metadata
    let metadata = client.get_file_metadata("test/hello.txt").await?;
    println!("Metadata: {:?}", metadata);

    // List all files
    let files = client.list_files(None).await?;
    println!("Files in storage:");
    for file in &files {
        println!("  - {}", file);
    }

    // Copy the file
    client
        .copy_file("test/hello.txt", "test/hello_copy.txt")
        .await?;
    println!("Copied to: test/hello_copy.txt");

    // Delete the files
    client.delete_file("test/hello.txt").await?;
    client.delete_file("test/hello_copy.txt").await?;
    println!("Cleaned up test files");

    Ok(())
}
