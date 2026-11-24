<div align="center">
  <img src="logo.svg" width="128" height="128" alt="Progressor Logo"/>
  <h1>Progressor</h1>
  <p>A modern, async-first progress tracking library for Rust.</p>
  
  [![Crates.io](https://img.shields.io/crates/v/progressor.svg)](https://crates.io/crates/progressor)
  [![Documentation](https://docs.rs/progressor/badge.svg)](https://docs.rs/progressor)
  [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
</div>

## Features

- **Async-first**: Built around Rust's `Future` and `Stream` APIs
- **Coalescing latest-value watch channel**: High-frequency updates overwrite stale snapshots in place with no ring overflow
- **Custom `Copy` snapshot types**: Parameterized `Progress::Update` with `watch` and `track` for zero-allocation steady-state updates
- **Optional totals**: Distinguish indeterminate progress (`None`) from zero-length work (`Some(0)`)
- **Explicit cancellation**: Early `Err` returns close the progress stream cleanly without fabricating `State::Cancelled`
- **Lightweight**: Minimal dependencies and fast compilation

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
progressor = "0.1"
```

### Simple Progress Monitoring (Recommended)

Using `ProgressExt::observe` with an `FnMut` observer:

```rust
use progressor::{ProgressExt, progress};

#[tokio::main]
async fn main() {
    let mut observed = 0_usize;
    let result = progress(Some(100), |mut updater| async move {
        for i in 0..=100 {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            if i % 10 == 0 {
                updater.update_with_message(i, format!("Processing step {i}/100"));
            } else {
                updater.update(i);
            }
        }
        updater.complete();
        "Task completed!"
    })
    .observe(|update| {
        observed += 1;
        match (update.completed_fraction(), update.total()) {
            (Some(fraction), Some(total)) => {
                println!(
                    "Progress: {:.1}% ({}/{})",
                    fraction * 100.0,
                    update.current(),
                    total
                );
            }
            _ => println!("Progress: {}", update.current()),
        }

        if let Some(message) = update.message() {
            println!("  {message}");
        }
    })
    .await;

    println!("Result: {result} ({observed} updates observed)");
}
```

### Custom Snapshot Types with `watch` and `track`

```rust
use progressor::{ProgressExt, track, watch};

#[derive(Clone, Copy, Debug)]
struct DownloadProgress {
    downloaded: u64,
    total: Option<u64>,
    active_workers: u16,
}

#[tokio::main]
async fn main() {
    let (tx, rx) = watch(None);
    let task = track(rx, async move {
        tx.send(DownloadProgress {
            downloaded: 4096,
            total: Some(8192),
            active_workers: 4,
        });
        tx.send(DownloadProgress {
            downloaded: 8192,
            total: Some(8192),
            active_workers: 0,
        });
        8192_u64
    });

    let bytes = task
        .observe(|snap| {
            println!(
                "Downloaded {} / {:?} bytes ({} workers)",
                snap.downloaded, snap.total, snap.active_workers
            );
        })
        .await;

    println!("Finished: {bytes} bytes");
}
```

### Advanced Manual Control

For more control over progress monitoring using streams:

```rust
use futures_util::StreamExt;
use progressor::{Progress, progress};

#[tokio::main]
async fn main() {
    let task = progress(Some(100), |mut updater| async move {
        for i in 0..=100 {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            if i % 10 == 0 {
                updater.update_with_message(i, format!("Processing step {i}/100"));
            } else {
                updater.update(i);
            }
        }
        updater.complete();
        "Task completed!"
    });

    let mut progress_stream = task.progress();

    tokio::select! {
        result = task => {
            println!("Result: {result}");
        }
        () = async {
            while let Some(update) = progress_stream.next().await {
                if let (Some(fraction), Some(total)) = (update.completed_fraction(), update.total()) {
                    println!(
                        "Progress: {:.1}% ({}/{})",
                        fraction * 100.0,
                        update.current(),
                        total
                    );
                }

                if let Some(message) = update.message() {
                    println!("  {message}");
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

### `Progress` Trait

Trait extending `Future` with an associated `type Update: Clone + Send + Sync + 'static` and `progress(&self) -> impl Stream<Item = Self::Update> + Unpin + Send + 'static`.

### `ProgressExt` Trait

Extension trait providing convenient methods:
- `observe(receiver)`: Monitor progress with an `FnMut(Self::Update) + Send` callback, draining any final ready snapshot before returning
- `observe_local(receiver)`: Local version that doesn't require `Send` bounds

### `watch()`, `WatchSender`, `WatchReceiver`, `track()`, `Tracked`

In-tree coalescing latest-value watch channel and future wrapper for attaching custom `Progress::Update` snapshots to any `Future`.

### `ProgressUpdate`

Built-in progress snapshot with:
- `current()`: Current progress value
- `total()`: Optional total progress value (`Option<u64>`)
- `state()`: Current state (`Working`, `Paused`, `Completed`, `Cancelled`)
- `message()`: Optional progress message
- `completed_fraction()`: Optional progress fraction (`Option<f64>`)
- `remaining()`: Optional remaining work (`Option<u64>`)

### `progress()` Function and `ProgressUpdater`

Creates a `Progress<Update = ProgressUpdate>` future from a closure receiving a `ProgressUpdater`:
- `update(current)`: Update progress value
- `update_with_message(current, message)`: Update with message
- `set_total(total)`: Set or clear (`None`) the expected total
- `pause()` / `pause_with_message(message)`: Pause the operation
- `complete()`: Mark the operation completed
- `cancel()`: Explicitly mark the operation cancelled

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.
