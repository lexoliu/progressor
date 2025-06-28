//! Basic example demonstrating progress tracking functionality.

use futures_util::StreamExt;
use progressor::{updater::progress, Progress};

#[tokio::main]
async fn main() {
    println!("Starting progress tracking example...");

    // Create a simulated long-running task
    let task = progress(100, |mut updater| async move {
        for i in 0..=100 {
            // Simulate some work
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            if i % 10 == 0 {
                updater.update_with_message(i, format!("Processing step {i}/100"));
            } else {
                updater.update(i);
            }
        }
        "Task completed successfully!"
    });

    // Get the progress stream
    let mut progress_stream = Box::pin(task.progress());

    // Run task and progress monitoring concurrently
    tokio::select! {
        result = task => {
            println!("\nTask result: {result}");
        }
        () = async {
            while let Some(update) = progress_stream.next().await {
                print!("\rProgress: {:.1}% ({}/{})",
                       update.completed_fraction() * 100.0,
                       update.current,
                       update.total);

                if let Some(message) = &update.message {
                    print!(" - {message}");
                }

                if update.is_complete() {
                    println!("\n✅ Progress completed!");
                    break;
                }
            }
        } => {}
    }
}
