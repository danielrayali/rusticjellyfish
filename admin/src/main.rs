use redis::AsyncCommands;
use serde_json::{json, Value};
use std::io::{self, Write};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to Redis
    let client = redis::Client::open("redis://127.0.0.1:6379/")?;
    let mut con = client.get_multiplexed_async_connection().await?;

    println!("Redis Client Manager");
    println!("==================");

    loop {
        println!("\nOptions:");
        println!("1. List all clients");
        println!("2. Add task to client");
        println!("3. View client details");
        println!("4. View task results");
        println!("5. Show task status summary");
        println!("6. Clear completed tasks");
        println!("7. Exit");

        print!("Enter your choice: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim() {
            "1" => list_clients(&mut con).await?,
            "2" => add_task(&mut con).await?,
            "3" => view_client_details(&mut con).await?,
            "4" => view_task_results(&mut con).await?,
            "5" => show_task_status_summary(&mut con).await?,
            "6" => clear_completed_tasks(&mut con).await?,
            "7" => break,
            _ => println!("Invalid choice. Please try again."),
        }
    }

    Ok(())
}

async fn list_clients(con: &mut redis::aio::MultiplexedConnection) -> Result<(), Box<dyn std::error::Error>> {
    // Get all keys matching client:*
    let keys: Vec<String> = con.keys("client:*").await?;

    if keys.is_empty() {
        println!("No clients registered.");
        return Ok(());
    }

    println!("\nRegistered Clients:");
    println!("===================");

    for key in keys {
        let client_data_str: Option<String> = con.get(&key).await?;

        if let Some(data_str) = client_data_str {
            match serde_json::from_str::<Value>(&data_str) {
                Ok(client_data) => {
                    let client_id = client_data["client_id"].as_str().unwrap_or("unknown");
                    let config_id = client_data["config_id"].as_str().unwrap_or("unknown");
                    let last_seen = client_data["last_seen"].as_str().unwrap_or("never");
                    let empty_vec = Vec::new();
                    let tasks_array = client_data["tasks"].as_array().unwrap_or(&empty_vec);
                    let pending_tasks = tasks_array.iter()
                        .filter(|task| task.get("status").and_then(|s| s.as_str()) == Some("pending"))
                        .count();
                    let completed_tasks = tasks_array.iter()
                        .filter(|task| task.get("status").and_then(|s| s.as_str()) == Some("completed"))
                        .count();
                    let total_tasks = tasks_array.len();

                    // Convert timestamp to readable format
                    let last_seen_readable = if let Ok(timestamp) = last_seen.parse::<i64>() {
                        chrono::DateTime::from_timestamp(timestamp, 0)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                            .unwrap_or_else(|| "Invalid timestamp".to_string())
                    } else {
                        last_seen.to_string()
                    };

                    println!("Client ID: {}", client_id);
                    println!("Config ID: {}", config_id);
                    println!("Last Seen: {}", last_seen_readable);
                    println!("Tasks: {} total ({} pending, {} completed)", total_tasks, pending_tasks, completed_tasks);
                    println!("---");
                }
                Err(e) => {
                    println!("Error parsing client data for {}: {}", key, e);
                }
            }
        }
    }

    Ok(())
}

async fn add_task(con: &mut redis::aio::MultiplexedConnection) -> Result<(), Box<dyn std::error::Error>> {
    print!("Enter client ID: ");
    io::stdout().flush()?;
    let mut client_id = String::new();
    io::stdin().read_line(&mut client_id)?;
    let client_id = client_id.trim();

    print!("Enter command to execute: ");
    io::stdout().flush()?;
    let mut command = String::new();
    io::stdin().read_line(&mut command)?;
    let command = command.trim();

    // Generate a unique task ID
    let task_id = Uuid::new_v4();

    // Look up the client
    let key = format!("client:{}", client_id);
    let client_data_str: Option<String> = con.get(&key).await?;

    if let Some(data_str) = client_data_str {
        let mut client_data: Value = serde_json::from_str(&data_str)?;

        // Create the task
        let task = json!({
            "task_id": task_id.to_string(),
            "command": command,
            "status": "pending",
            "created_at": chrono::Utc::now().timestamp().to_string(),
            "completed_at": null,
            "return_code": null,
            "stdout": null,
            "stderr": null
        });

        // Add task to the tasks array
        if let Some(tasks) = client_data["tasks"].as_array_mut() {
            tasks.push(task);
        } else {
            client_data["tasks"] = json!([task]);
        }

        // Update the client data in Redis
        let _: () = con.set(&key, client_data.to_string()).await?;

        println!("Task added successfully!");
        println!("Task ID: {}", task_id);
        println!("Command: {}", command);
        println!("Client will receive this task on next check-in.");
    } else {
        println!("Client not found: {}", client_id);
    }

    Ok(())
}

async fn view_client_details(con: &mut redis::aio::MultiplexedConnection) -> Result<(), Box<dyn std::error::Error>> {
    print!("Enter client ID: ");
    io::stdout().flush()?;
    let mut client_id = String::new();
    io::stdin().read_line(&mut client_id)?;
    let client_id = client_id.trim();

    let key = format!("client:{}", client_id);
    let client_data_str: Option<String> = con.get(&key).await?;

    if let Some(data_str) = client_data_str {
        let client_data: Value = serde_json::from_str(&data_str)?;

        println!("\nClient Details:");
        println!("===============");
        println!("{}", serde_json::to_string_pretty(&client_data)?);
    } else {
        println!("Client not found: {}", client_id);
    }

    Ok(())
}

async fn view_task_results(con: &mut redis::aio::MultiplexedConnection) -> Result<(), Box<dyn std::error::Error>> {
    print!("Enter client ID: ");
    io::stdout().flush()?;
    let mut client_id = String::new();
    io::stdin().read_line(&mut client_id)?;
    let client_id = client_id.trim();

    let key = format!("client:{}", client_id);
    let client_data_str: Option<String> = con.get(&key).await?;

    if let Some(data_str) = client_data_str {
        let client_data: Value = serde_json::from_str(&data_str)?;

        if let Some(tasks) = client_data["tasks"].as_array() {
            if tasks.is_empty() {
                println!("No tasks found for this client.");
                return Ok(());
            }

            println!("\nTask Results:");
            println!("=============");

            for (i, task) in tasks.iter().enumerate() {
                let task_id = task["task_id"].as_str().unwrap_or("unknown");
                let command = task["command"].as_str().unwrap_or("unknown");
                let status = task["status"].as_str().unwrap_or("unknown");
                let return_code = task["return_code"].as_i64();
                let stdout = task["stdout"].as_str().unwrap_or("");
                let stderr = task["stderr"].as_str().unwrap_or("");

                println!("Task #{}: {}", i + 1, task_id);
                println!("Command: {}", command);
                println!("Status: {}", status);

                if let Some(rc) = return_code {
                    println!("Return Code: {}", rc);
                    println!("STDOUT: {}", stdout);
                    println!("STDERR: {}", stderr);
                }

                println!("---");
            }
        } else {
            println!("No tasks found for this client.");
        }
    } else {
        println!("Client not found: {}", client_id);
    }

    Ok(())
}

async fn show_task_status_summary(con: &mut redis::aio::MultiplexedConnection) -> Result<(), Box<dyn std::error::Error>> {
    print!("Enter client ID: ");
    io::stdout().flush()?;
    let mut client_id = String::new();
    io::stdin().read_line(&mut client_id)?;
    let client_id = client_id.trim();

    let key = format!("client:{}", client_id);
    let client_data_str: Option<String> = con.get(&key).await?;

    if let Some(data_str) = client_data_str {
        let client_data: Value = serde_json::from_str(&data_str)?;

        if let Some(tasks) = client_data["tasks"].as_array() {
            if tasks.is_empty() {
                println!("No tasks found for this client.");
                return Ok(());
            }

            let mut pending_count = 0;
            let mut completed_count = 0;
            let mut failed_count = 0;
            let mut unknown_count = 0;

            println!("\nTask Status Summary for Client: {}", client_id);
            println!("==========================================");

            for task in tasks {
                let task_id = task["task_id"].as_str().unwrap_or("unknown");
                let command = task["command"].as_str().unwrap_or("unknown");
                let status = task["status"].as_str().unwrap_or("unknown");

                match status {
                    "pending" => {
                        pending_count += 1;
                        println!("⏳ PENDING  - {} - {}", task_id, command);
                    }
                    "completed" => {
                        completed_count += 1;
                        let return_code = task["return_code"].as_i64().unwrap_or(-999);
                        if return_code == 0 {
                            println!("✅ COMPLETED - {} - {} (exit code: {})", task_id, command, return_code);
                        } else {
                            println!("❌ COMPLETED - {} - {} (exit code: {})", task_id, command, return_code);
                        }
                    }
                    "failed" => {
                        failed_count += 1;
                        println!("💥 FAILED   - {} - {}", task_id, command);
                    }
                    _ => {
                        unknown_count += 1;
                        println!("❓ UNKNOWN  - {} - {} (status: {})", task_id, command, status);
                    }
                }
            }

            println!("\n📊 Summary:");
            println!("  Pending: {}", pending_count);
            println!("  Completed: {}", completed_count);
            println!("  Failed: {}", failed_count);
            println!("  Unknown: {}", unknown_count);
            println!("  Total: {}", tasks.len());

            // Show what would be sent to client
            println!("\n🔄 Tasks that would be sent to client: {}", pending_count);

        } else {
            println!("No tasks found for this client.");
        }
    } else {
        println!("Client not found: {}", client_id);
    }

    Ok(())
}

async fn clear_completed_tasks(con: &mut redis::aio::MultiplexedConnection) -> Result<(), Box<dyn std::error::Error>> {
    print!("Enter client ID: ");
    io::stdout().flush()?;
    let mut client_id = String::new();
    io::stdin().read_line(&mut client_id)?;
    let client_id = client_id.trim();

    let key = format!("client:{}", client_id);
    let client_data_str: Option<String> = con.get(&key).await?;

    if let Some(data_str) = client_data_str {
        let mut client_data: Value = serde_json::from_str(&data_str)?;

        if let Some(tasks) = client_data["tasks"].as_array_mut() {
            let original_count = tasks.len();

            // Keep only pending tasks
            tasks.retain(|task| task["status"].as_str() == Some("pending"));

            let removed_count = original_count - tasks.len();

            // Update the client data in Redis
            let _: () = con.set(&key, client_data.to_string()).await?;

            println!("Cleared {} completed tasks.", removed_count);
        } else {
            println!("No tasks found for this client.");
        }
    } else {
        println!("Client not found: {}", client_id);
    }

    Ok(())
}
