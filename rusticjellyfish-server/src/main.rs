use axum::{
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use redis::AsyncCommands;
use serde_json::{json, Value};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the router with the admin endpoint and register endpoint
    let app = Router::new()
        .route("/admin/get-clients", get(get_clients_handler))
        .route("/register", post(register_handler));

    // Create a TCP listener on localhost:3000
    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    
    println!("Server running on http://127.0.0.1:3000");
    println!("Try: curl http://127.0.0.1:3000/admin/get-clients");
    println!("Try: curl -X POST http://127.0.0.1:3000/register");

    // Start the server
    axum::serve(listener, app).await?;

    Ok(())
}

// Handler function for the /admin/get-clients endpoint
async fn get_clients_handler() -> Result<Json<Value>, StatusCode> {
    // Return a simple JSON response with 200 OK
    let response = json!({
        "status": "success",
        "message": "Clients retrieved successfully",
        "clients": []
    });
    
    Ok(Json(response))
}

// Handler function for the /register endpoint
async fn register_handler() -> Result<StatusCode, StatusCode> {
    // Connect to Redis
    let client = redis::Client::open("redis://127.0.0.1:6379/")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let mut con = client.get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Create an empty JSON string
    let empty_json = "{}";
    
    // Generate a unique key for this registration (you might want to use a UUID or timestamp)
    let key = format!("registration:{}", chrono::Utc::now().timestamp_millis());
    
    // Store the empty JSON string in Redis
    let _: () = con.set(&key, empty_json)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    println!("Stored empty JSON at key: {}", key);
    
    Ok(StatusCode::OK)
}