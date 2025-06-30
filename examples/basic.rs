//! Basic example demonstrating progress tracking functionality.
use futures_util::StreamExt;
use progressor::{Progress, State, progress};

#[tokio::main]
async fn main() {
    println!("Starting progress tracking example...");

    // Create a simulated long-running task
    let task = progress(100, |mut updater| async move {
        for i in 0u64..=100 {
            // Simulate some work
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            if i.is_multiple_of(10) {
                updater.update_with_message(i, format!("Processing step {i}/100"));
            } else {
                updater.update(i);
            }
        }
        "Task completed successfully!"
    });

    // Get the progress stream - no need for Box::pin with Unpin
    let mut progress_stream = task.progress();

    // Run task and progress monitoring concurrently
    tokio::select! {
        result = task => {
            println!("\nTask result: {result}");
        }
        () = async {
            while let Some(update) = progress_stream.next().await {
                print!("\rProgress: {:.1}% ({}/{})",
                       update.completed_fraction() * 100.0,
                       update.current(),
                       update.total());

                if let Some(message) = update.message() {
                    print!(" - {message}");
                }

                match update.state() {
                    State::Completed => {
                        println!("\n✅ Progress completed!");
                        break;
                    }
                    State::Cancelled => {
                        println!("\n❌ Progress was cancelled!");
                        break;
                    }
                    _ => {}
                }
            }
        } => {}
    }
}
