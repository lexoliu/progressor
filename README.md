# Progressor

A modern, async-first progress tracking library for Rust.

## Features

- **Async-first**: Built around Rust's async/await and Stream APIs
- **Zero-allocation progress updates**: Efficient progress reporting
- **Flexible progress tracking**: Support for current/total, messages, and cancellation
- **Type-safe**: Full Rust type safety with meaningful error messages
- **Lightweight**: Minimal dependencies and fast compilation

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
progressor = "0.0.1"
```

### Basic Example

```rust
use progressor::{Progress, updater::progress};
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    // Create a progress-tracked task
    let task = progress(100, |mut updater| async move {
        for i in 0..=100 {
            // Simulate work
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            
            // Update progress
            if i % 10 == 0 {
                updater.update_with_message(i, format!("Processing step {i}/100"));
            } else {
                updater.update(i);
            }
        }
        "Task completed!"
    });

    // Monitor progress concurrently
    let mut progress_stream = Box::pin(task.progress());
    
    tokio::select! {
        result = task => {
            println!("Result: {result}");
        }
        _ = async {
            while let Some(update) = progress_stream.next().await {
                println!("Progress: {:.1}% ({}/{})", 
                         update.completed_fraction() * 100.0,
                         update.current, 
                         update.total);
                
                if update.is_complete() {
                    break;
                }
            }
        } => {}
    }
}
```

## API Reference

### `ProgressUpdate`

Represents a single progress update with:
- `current`: Current progress value
- `total`: Total progress value  
- `is_cancelled`: Whether the operation was cancelled
- `message`: Optional progress message

### `Progress` Trait

Trait for types that can report progress via a `Stream` of `ProgressUpdate`s.

### `progress()` Function

Creates a progress-tracked future from a closure that receives a `ProgressUpdater`.

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.
