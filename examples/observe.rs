//! Example demonstrating the convenient observe extension.

use progressor::{ProgressExt, State, progress};

#[tokio::main]
async fn main() {
    println!("Starting observe extension example...");

    // Create a simulated long-running task using the observe extension
    let result = progress(100, |mut updater| async move {
        for i in 0u64..=100 {
            // Simulate some work
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            if i.is_multiple_of(10) {
                updater.update_with_message(i, format!("Processing step {i}/100"));
            } else {
                updater.update(i);
            }

            // Demonstrate pausing
            if i == 50 {
                updater.pause();
                println!("\n⏸️  Pausing for a moment...");
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                updater.update_with_message(i, "Resuming...".to_string());
            }
        }
        "Task completed successfully!"
    })
    .observe(|update| {
        // This closure is called for each progress update
        print!(
            "\rProgress: {:.1}% ({}/{})",
            update.completed_fraction() * 100.0,
            update.current(),
            update.total()
        );

        if let Some(message) = update.message() {
            print!(" - {message}");
        }

        match update.state() {
            State::Working => {
                // Normal progress, already printed above
            }
            State::Paused => {
                print!(" [PAUSED]");
            }
            State::Completed => {
                println!("\n✅ Progress completed!");
            }
            State::Cancelled => {
                println!("\n❌ Progress was cancelled!");
            }
        }
    })
    .await;

    println!("\nTask result: {result}");
}
