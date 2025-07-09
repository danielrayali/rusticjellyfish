use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{Json, Response},
    routing::get,
    Router,
};
use redis::AsyncCommands;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the router with just the register endpoint
    let app = Router::new()
        .route("/register", get(register_handler))
        .layer(middleware::from_fn(logging_middleware));

    // Create a TCP listener on localhost:3000
    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    
    println!("Server running on http://127.0.0.1:3000");
    println!("Try: curl http://127.0.0.1:3000/register");

    // Start the server
    axum::serve(listener, app).await?;

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
        "registered_at": chrono::Utc::now().to_rfc3339()
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