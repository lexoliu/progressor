<div align="center">
  <img src="logo.svg" width="128" height="128" alt="Progressor Logo"/>
  <h1>Progressor</h1>
  <p>A modern, async-first progress tracking library for Rust.</p>
  
  [![Crates.io](https://img.shields.io/crates/v/progressor.svg)](https://crates.io/crates/progressor)
  [![Documentation](https://docs.rs/progressor/badge.svg)](https://docs.rs/progressor)
  [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
</div>

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

### Simple Progress Monitoring (Recommended)

Using the `observe` extension method for easy progress monitoring:

```rust
use progressor::{progress, ProgressExt};

#[tokio::main]
async fn main() {
    let result = progress(100, |mut updater| async move {
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
    })
    .observe(|update| {
        println!("Progress: {:.1}% ({}/{})", 
                 update.completed_fraction() * 100.0,
                 update.current(), 
                 update.total());
        
        if let Some(message) = update.message() {
            println!("  {}", message);
        }
    })
    .await;

    println!("Result: {}", result);
}
```

### Advanced Manual Control

For more control over progress monitoring using streams:

```rust
use progressor::{progress, Progress};
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
    let mut progress_stream = task.progress();
    
    tokio::select! {
        result = task => {
            println!("Result: {}", result);
        }
        _ = async {
            while let Some(update) = progress_stream.next().await {
                println!("Progress: {:.1}% ({}/{})", 
                         update.completed_fraction() * 100.0,
                         update.current(), 
                         update.total());
                
                if let Some(message) = update.message() {
                    println!("  {}", message);
                }
                
                if update.is_completed() {
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
- `current()`: Current progress value
- `total()`: Total progress value  
- `state()`: Current state (Working, Paused, Completed, Cancelled)
- `message()`: Optional progress message
- `completed_fraction()`: Progress as a fraction (0.0 to 1.0)
- `remaining()`: Remaining work (total - current)

### `Progress` Trait

Trait for types that can report progress via a `Stream` of `ProgressUpdate`s.

### `ProgressExt` Trait

Extension trait providing convenient methods:
- `observe(receiver)`: Monitor progress with a callback function
- `observe_local(receiver)`: Local version that doesn't require `Send` bounds

### `progress()` Function

Creates a progress-tracked future from a closure that receives a `ProgressUpdater`.

### `ProgressUpdater`

Handle for updating progress during execution:
- `update(current)`: Update progress value
- `update_with_message(current, message)`: Update with message
- `pause()`: Pause the operation
- `cancel()`: Cancel the operation

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.
