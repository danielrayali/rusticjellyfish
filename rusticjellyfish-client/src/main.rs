use reqwest::Client;
use std::process::Command;
use std::str;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the current git commit hash
    let commit_hash = get_current_commit_hash()?;
    
    println!("Current commit hash: {}", commit_hash);
    
    // Create HTTP client
    let client = Client::new();
    
    // Make POST request to /register endpoint with Build-Id header
    let response = client
        .post("http://127.0.0.1:3000/register")
        .header("Build-Id", commit_hash)
        .send()
        .await?;
    
    println!("Response status: {}", response.status());
    println!("Response headers: {:#?}", response.headers());
    
    // Check if the request was successful
    if response.status().is_success() {
        println!("Registration successful!");
    } else {
        println!("Registration failed with status: {}", response.status());
    }
    
    Ok(())
}

fn get_current_commit_hash() -> Result<String, Box<dyn std::error::Error>> {
    // Execute git command to get current commit hash
    let output = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()?;
    
    if !output.status.success() {
        return Err("Failed to get git commit hash. Make sure you're in a git repository.".into());
    }
    
    // Convert output to string and trim whitespace
    let commit_hash = str::from_utf8(&output.stdout)?
        .trim()
        .to_string();
    
    Ok(commit_hash)
}