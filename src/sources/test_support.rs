use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn server(replies: Vec<(u16, Value)>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("address"));
    let task = tokio::spawn(async move {
        let mut requests = vec![];
        for (status, value) in replies {
            let (mut socket, _) =
                tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
                    .await
                    .expect("request timeout")
                    .expect("accept");
            let mut data = vec![];
            loop {
                let mut buffer = [0; 4096];
                let count = socket.read(&mut buffer).await.expect("read");
                assert!(count > 0);
                data.extend_from_slice(&buffer[..count]);
                let raw = String::from_utf8_lossy(&data);
                if let Some(end) = raw.find("\r\n\r\n") {
                    let length: usize = raw[..end]
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|s| s.parse().ok())
                        })
                        .unwrap_or(0);
                    if data.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            requests.push(String::from_utf8(data).expect("HTTP utf8"));
            let body = value.to_string();
            let response = format!(
                "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nLocation: http://127.0.0.1:1/should-not-follow\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("respond");
        }
        requests
    });
    (base, task)
}
