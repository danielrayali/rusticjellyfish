use reqwest::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the current git commit hash
    // let config_id = "0d02473e-52c1-434c-ac68-6cfe4d18d50f";
    let config_id = "@JELLYFISH_CONFIG_ID@###############";
    
    println!("Config ID: {}", config_id);
    
    // Create HTTP client
    let client = Client::new();
    
    // Make GET request to /register endpoint with Build-Id header
    let response = client
        .get("http://127.0.0.1:3000/register")
        .header("Config-Id", config_id)
        .send()
        .await?;
    
    println!("Response status: {}", response.status());
    println!("Response headers: {:#?}", response.headers());
    
    // Check if the request was successful
    if response.status().is_success() {
        // Parse and print the JSON response
        match response.json::<serde_json::Value>().await {
            Ok(json) => {
                print!("Response JSON: ");
                println!("{}", serde_json::to_string_pretty(&json).unwrap());
                
                // You can also access specific fields
                if let Some(client_id) = json.get("client_id") {
                    println!("Client ID: {}", client_id);
                }
                if let Some(config_id) = json.get("config_id") {
                    println!("Config ID: {}", config_id);
                }
            }
            Err(e) => {
                println!("Failed to parse JSON response: {}", e);
            }
        }
        println!("Registration successful!");
    } else {
        println!("Registration failed with status: {}", response.status());
        // Try to read error response as text
        match response.text().await {
            Ok(text) => println!("Error response: {}", text),
            Err(e) => println!("Could not read error response: {}", e),
        }
    }
    
    Ok(())
}
