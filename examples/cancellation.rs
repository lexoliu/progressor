//! Example demonstrating progress tracking with cancellation and pause support.
use futures_util::StreamExt;
use progressor::{Progress, State, progress};

#[tokio::main]
async fn main() {
    println!("Starting cancellation example...");

    // Create a task that can be paused and cancelled
    let task = progress(Some(100), |mut updater| async move {
        for i in 0u64..=100 {
            // Simulate some work
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            if i.is_multiple_of(10) {
                updater.update_with_message(i, format!("Processing step {i}/100"));
            } else {
                updater.update(i);
            }

            // Pause at 30%
            if i == 30 {
                println!("\n⏸️  Pausing task at 30%...");
                updater.pause();
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                println!("▶️  Resuming task...");
            }

            // Cancel after reaching 60%
            if i >= 60 {
                println!("\n⚠️  Cancelling task...");
                updater.cancel();
                return "Task cancelled by user";
            }
        }
        updater.complete();
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
                if let (Some(fraction), Some(total)) = (update.completed_fraction(), update.total()) {
                    print!(
                        "\rProgress: {:.1}% ({}/{total})",
                        fraction * 100.0,
                        update.current()
                    );
                } else {
                    print!("\rProgress: {}", update.current());
                }

                if let Some(message) = update.message() {
                    print!(" - {message}");
                }

                match update.state() {
                    State::Working => {
                        // Continue normal progress display
                    }
                    State::Paused => {
                        print!(" [PAUSED]");
                    }
                    State::Completed => {
                        println!("\n✅ Progress completed!");
                        break;
                    }
                    State::Cancelled => {
                        println!("\n❌ Progress was cancelled!");
                        break;
                    }
                }
            }
        } => {}
    }
}
