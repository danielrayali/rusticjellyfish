use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{Json, Response},
    routing::{get, post},
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use redis::AsyncCommands;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configuration
    let use_https = std::env::var("USE_HTTPS").unwrap_or_else(|_| "false".to_string()) == "true";
    let cert_path = std::env::var("CERT_PATH").unwrap_or_else(|_| "cert.pem".to_string());
    let key_path = std::env::var("KEY_PATH").unwrap_or_else(|_| "key.pem".to_string());
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

    // Create the router with all endpoints
    let app = Router::new()
        .route("/register", get(register_handler))
        .route("/tasking", get(tasking_handler))
        .route("/task_result", post(task_result_handler))
        .layer(middleware::from_fn(logging_middleware));

    println!("Server configuration:");
    println!("  HTTPS: {}", use_https);
    println!("  Bind address: {}", bind_addr);

    if use_https {
        println!("  Certificate: {}", cert_path);
        println!("  Private key: {}", key_path);

        // Configure TLS
        let config = RustlsConfig::from_pem_file(
            PathBuf::from(cert_path),
            PathBuf::from(key_path),
        )
        .await?;

        println!("Available endpoints:");
        println!("  GET  /register     - Register a new client");
        println!("  GET  /tasking      - Get tasks for a client");
        println!("  POST /task_result  - Submit task results");

        // Start HTTPS server
        axum_server::bind_rustls(bind_addr.parse()?, config)
            .serve(app.into_make_service())
            .await?;
    } else {
        // Create a TCP listener
        let listener = TcpListener::bind(&bind_addr).await?;

        println!("Available endpoints:");
        println!("  GET  /register     - Register a new client");
        println!("  GET  /tasking      - Get tasks for a client");
        println!("  POST /task_result  - Submit task results");

        // Start HTTP server
        axum::serve(listener, app).await?;
    }

    Ok(())
}

// Middleware to log request URL and headers
async fn logging_middleware(request: Request, next: Next) -> Response {
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    let method = request.method().clone();

    println!("=== Incoming Request ===");
    println!("Method: {}", method);
    println!("URL: {}", uri);
    println!("Headers:");
    for (name, value) in headers.iter() {
        println!("  {}: {:?}", name, value);
    }
    println!("========================");

    next.run(request).await
}

// Handler function for the /register endpoint
async fn register_handler(headers: HeaderMap) -> Result<Json<Value>, StatusCode> {
    // Connect to Redis
    let client = redis::Client::open("redis://127.0.0.1:6379/")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut con = client.get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Generate a unique UUID for this client
    let client_uuid = Uuid::new_v4();

    // Extract Config-Id from headers
    let config_id = headers.get("Config-Id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    // Create JSON with config_id and client UUID
    let client_data = json!({
        "client_id": client_uuid.to_string(),
        "config_id": config_id,
        "last_seen": chrono::Utc::now().timestamp().to_string(),
        "tasks": []  // Initialize empty tasks array
    });

    // Use the UUID as the Redis key
    let key = format!("client:{}", client_uuid);

    // Store the JSON with client data in Redis
    let _: () = con.set(&key, client_data.to_string())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    println!("Registered client {} with config_id: {}", client_uuid, config_id);

    // Return JSON response with the client UUID
    let response = json!({
        "status": "success",
        "message": "Client registered successfully",
        "client_id": client_uuid.to_string(),
        "config_id": config_id
    });

    Ok(Json(response))
}

// Handler function for the /tasking endpoint
async fn tasking_handler(headers: HeaderMap) -> Result<Json<Value>, StatusCode> {
    // Extract Client-Id from headers
    let client_id = headers.get("Client-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Connect to Redis
    let client = redis::Client::open("redis://127.0.0.1:6379/")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut con = client.get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Look up the client data using the client_id
    let key = format!("client:{}", client_id);
    let client_data_str: Option<String> = con.get(&key)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if client exists
    let client_data_str = client_data_str.ok_or(StatusCode::NOT_FOUND)?;

    // Parse the JSON from Redis
    let mut client_data: Value = serde_json::from_str(&client_data_str)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Update the last_seen field
    client_data["last_seen"] = Value::String(chrono::Utc::now().timestamp().to_string());

    let _: () = con.set(&key, client_data.to_string())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Extract and filter tasks - only send pending tasks
    let empty_vec = Vec::new();
    let all_tasks = client_data.get("tasks")
        .and_then(|t| t.as_array())
        .unwrap_or(&empty_vec);

    // Filter to only include pending tasks
    let pending_tasks: Vec<&Value> = all_tasks.iter()
        .filter(|task| {
            task.get("status")
                .and_then(|s| s.as_str())
                .map(|status| status == "pending")
                .unwrap_or(false)
        })
        .collect();

    println!("All tasks for client {}: {} total", client_id, all_tasks.len());
    println!("Pending tasks for client {}: {} pending", client_id, pending_tasks.len());

    // Return only the pending tasks
    let response = json!({
        "status": "success",
        "client_id": client_id,
        "tasks": pending_tasks
    });

    Ok(Json(response))
}

// Handler function for the /task_result endpoint
async fn task_result_handler(headers: HeaderMap, Json(payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    // Extract Client-Id from headers
    let client_id = headers.get("Client-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Extract task details from payload
    let task_id = payload.get("task_id")
        .and_then(|t| t.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let return_code = payload.get("return_code")
        .and_then(|r| r.as_i64())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let stdout = payload.get("stdout")
        .and_then(|s| s.as_str())
        .unwrap_or("");

    let stderr = payload.get("stderr")
        .and_then(|s| s.as_str())
        .unwrap_or("");

    let completed_at = payload.get("completed_at")
        .and_then(|c| c.as_str())
        .unwrap_or("");

    // Connect to Redis
    let client = redis::Client::open("redis://127.0.0.1:6379/")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut con = client.get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Look up the client data using the client_id
    let key = format!("client:{}", client_id);
    let client_data_str: Option<String> = con.get(&key)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if client exists
    let client_data_str = client_data_str.ok_or(StatusCode::NOT_FOUND)?;

    // Parse the JSON from Redis
    let mut client_data: Value = serde_json::from_str(&client_data_str)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Find and update the specific task
    let mut task_found = false;

    if let Some(tasks) = client_data["tasks"].as_array_mut() {
        for task in tasks.iter_mut() {
            if task.get("task_id").and_then(|t| t.as_str()) == Some(task_id) {
                // Update the task with results
                task["status"] = Value::String("completed".to_string());
                task["return_code"] = Value::Number(return_code.into());
                task["stdout"] = Value::String(stdout.to_string());
                task["stderr"] = Value::String(stderr.to_string());
                task["completed_at"] = Value::String(completed_at.to_string());

                task_found = true;
                break;
            }
        }
    }

    if !task_found {
        return Err(StatusCode::NOT_FOUND);
    }

    // Update the client data in Redis
    let _: () = con.set(&key, client_data.to_string())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    println!("Task {} completed for client {} with return code: {}",
             task_id, client_id, return_code);

    // Return success response
    let response = json!({
        "status": "success",
        "message": "Task result received successfully",
        "task_id": task_id,
        "client_id": client_id,
        "return_code": return_code
    });

    Ok(Json(response))
}