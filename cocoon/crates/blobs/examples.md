# s3 Examples

## Prerequisites

Need to work out how the permissions are exactly working. This code works on workspaces, but local does requure getting a token.

## Running S3 in a Mock Environment

This is possible with something like [MiniStack](https://github.com/ministackorg/ministack) which is worth looking into as the way of doing the testing for this. I would want to make sure that podman is fully firing across everything I do first though.

## To Do:

1. https://docs.rs/tokio/latest/tokio/fs/index.html - local playgound.

## Basic Usage

### Creating an S3 Client

```rust
use s3::S3Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client with default region (us-east-1)
    let client = S3Client::new("my-bucket-name".to_string()).await?;

    Ok(())
}
```

### Creating an S3 Client with Specific Region

```rust
use s3::S3Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = S3Client::new_with_region(
        "my-bucket-name".to_string(),
        "us-west-2".to_string()
    ).await?;

    Ok(())
}
```

## Uploading Files

### Upload from Memory

```rust
use s3::S3Client;
use bytes::Bytes;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = S3Client::new("my-bucket".to_string()).await?;

    let data = Bytes::from("Hello, S3!");
    client.upload_object("path/to/file.txt", data).await?;

    println!("File uploaded successfully!");
    Ok(())
}
```

### Upload from File Path

```rust
use s3::S3Client;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = S3Client::new("my-bucket".to_string()).await?;

    let file_path = Path::new("./local/file.parquet");
    client.upload_object_from_path("remote/file.parquet", file_path).await?;

    println!("File uploaded from path!");
    Ok(())
}
```

### Upload Large File (Multipart Upload)

For files larger than 5MB, use multipart upload for better reliability:

```rust
use s3::S3Client;
use bytes::Bytes;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = S3Client::new("my-bucket".to_string()).await?;

    // Load large file data
    let large_data = tokio::fs::read("./large_file.parquet").await?;
    let data = Bytes::from(large_data);

    // Automatically uses multipart upload for files > 5MB
    client.upload_large_file("data/large_file.parquet", data).await?;

    println!("Large file uploaded successfully!");
    Ok(())
}
```

## Downloading Files

### Download to Memory

```rust
use s3::S3Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = S3Client::new("my-bucket".to_string()).await?;

    let data = client.download_file("path/to/file.txt").await?;
    let content = String::from_utf8(data)?;

    println!("File content: {}", content);
    Ok(())
}
```

### Download to File Path

```rust
use s3::S3Client;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = S3Client::new("my-bucket".to_string()).await?;

    let local_path = Path::new("./downloads/file.parquet");
    client.download_file_to_path("remote/file.parquet", local_path).await?;

    println!("File downloaded to {:?}", local_path);
    Ok(())
}
```

## Listing Files

```rust
use s3::S3Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = S3Client::new("my-bucket".to_string()).await?;

    let files = client.list_files().await?;

    println!("Files in bucket:");
    for file in files {
        println!("  - {}", file);
    }

    Ok(())
}
```

## Checking File Existence

```rust
use s3::S3Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = S3Client::new("my-bucket".to_string()).await?;

    let exists = client.file_exists("path/to/file.txt").await?;

    if exists {
        println!("File exists!");
    } else {
        println!("File does not exist.");
    }

    Ok(())
}
```

## Getting File Metadata

```rust
use s3::S3Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = S3Client::new("my-bucket".to_string()).await?;

    let metadata = client.get_file_metadata("path/to/file.txt").await?;

    println!("Content Length: {} bytes", metadata.content_length);
    println!("Content Type: {:?}", metadata.content_type);
    println!("Last Modified: {:?}", metadata.last_modified);
    println!("ETag: {:?}", metadata.e_tag);

    Ok(())
}
```

## Copying Files

```rust
use s3::S3Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = S3Client::new("my-bucket".to_string()).await?;

    client.copy_file(
        "source/file.txt",
        "destination/file.txt"
    ).await?;

    println!("File copied successfully!");
    Ok(())
}
```

## Deleting Files

```rust
use s3::S3Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = S3Client::new("my-bucket".to_string()).await?;

    client.delete_file("path/to/file.txt").await?;

    println!("File deleted successfully!");
    Ok(())
}
```

## Complete Example: Upload, Download, and Clean Up

```rust
use s3::S3Client;
use bytes::Bytes;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = S3Client::new("my-bucket".to_string()).await?;

    // Upload a file
    let data = Bytes::from("Sample data for S3");
    let s3_key = "temp/sample.txt";

    println!("Uploading file...");
    client.upload_object(s3_key, data).await?;

    // Check if it exists
    let exists = client.file_exists(s3_key).await?;
    println!("File exists: {}", exists);

    // Get metadata
    let metadata = client.get_file_metadata(s3_key).await?;
    println!("File size: {} bytes", metadata.content_length);

    // Download it back
    println!("Downloading file...");
    let downloaded_data = client.download_file(s3_key).await?;
    println!("Downloaded content: {}", String::from_utf8(downloaded_data)?);

    // Clean up
    println!("Deleting file...");
    client.delete_file(s3_key).await?;
    println!("Done!");

    Ok(())
}
```

## Using with Environment Variables

```rust
use s3::S3Client;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read bucket name from environment
    let bucket = env::var("S3_BUCKET")
        .expect("S3_BUCKET environment variable not set");

    let client = S3Client::new(bucket).await?;

    // Use the client...
    let files = client.list_files().await?;
    println!("Found {} files", files.len());

    Ok(())
}
```

## Error Handling

All methods return `Result<T, Box<dyn std::error::Error>>`. Here's an example of proper error handling:

```rust
use s3::S3Client;

#[tokio::main]
async fn main() {
    let client = match S3Client::new("my-bucket".to_string()).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create S3 client: {}", e);
            return;
        }
    };

    match client.download_file("path/to/file.txt").await {
        Ok(data) => {
            println!("Successfully downloaded {} bytes", data.len());
        }
        Err(e) => {
            eprintln!("Failed to download file: {}", e);
        }
    }
}
```
