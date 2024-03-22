use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio::signal::ctrl_c;
use tokio::sync::Notify;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("localhost:3000").await.unwrap();
    let state = Arc::new((AtomicUsize::new(0), Notify::new()));

    loop {
        select! {
            result = listener.accept() => {
                let (connection, _) = result.unwrap();
                let state = state.clone();

                state.0.fetch_add(1, Ordering::Relaxed);

                tokio::spawn(async move {
                    if let Err(e) = handle_connection(connection).await {
                        println!("failed to handle connection: {e}")
                    }

                    let count = state.0.fetch_sub(1, Ordering::Relaxed);
                    if count == 1 {
                        state.1.notify_one();
                    }
                });
            }
            _shutdown = ctrl_c() => {
                let timer = tokio::time::sleep(Duration::from_secs(30));
                let request_counter = state.1.notified();

                if state.0.load(Ordering::Relaxed) != 0 {
                    select! {
                        _ = timer => {}
                        _ = request_counter => {}
                    }
                }

                println!("Gracefully shutting down.");
                return;
            }
        }
    }
}

async fn handle_connection(mut conn: TcpStream) -> io::Result<()> {
    let mut read = 0;
    let mut request = [0u8; 1024];

    loop {
        let num_bytes = conn.read(&mut request[read..]).await?;

        if num_bytes == 0 {
            println!("client disconnected");
            return Ok(());
        }

        read += num_bytes;

        if request.get(read - 4..read) == Some(b"\r\n\r\n") {
            break;
        }
    }
    let request = String::from_utf8_lossy(&request[..read]);
    println!("{request}");

    let response = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Length: 12\n",
        "Connection: close\r\n\r\n",
        "Hello world!"
    );
    let mut written = 0;
    loop {
        let num_bytes = conn.write(response[written..].as_bytes()).await?;

        if num_bytes == 0 {
            println!("client disconnected unexpect");
            return Ok(());
        }

        written += num_bytes;
        if written == response.len() {
            break;
        }
    }

    conn.flush().await
}
