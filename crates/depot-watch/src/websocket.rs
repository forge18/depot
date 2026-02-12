use depot_core::{DepotError, DepotResult};
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// WebSocket server for browser reload
pub struct WebSocketServer {
    port: u16,
    should_stop: Arc<AtomicBool>,
    reload_tx: broadcast::Sender<()>,
}

impl WebSocketServer {
    pub fn new(port: u16) -> Self {
        let (reload_tx, _) = broadcast::channel(16);
        Self {
            port,
            should_stop: Arc::new(AtomicBool::new(false)),
            reload_tx,
        }
    }

    /// Start the WebSocket server
    pub async fn start(&self) -> DepotResult<()> {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| DepotError::Package(format!("Failed to bind WebSocket server: {}", e)))?;

        println!("🌐 WebSocket server listening on ws://{}", addr);

        let should_stop = Arc::clone(&self.should_stop);
        let reload_tx = self.reload_tx.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, _)) => {
                                let reload_rx = reload_tx.subscribe();
                                tokio::spawn(handle_client(stream, reload_rx));
                            }
                            Err(e) => {
                                eprintln!("WebSocket accept error: {}", e);
                            }
                        }
                    }
                    _ = tokio::signal::ctrl_c() => {
                        break;
                    }
                }

                if should_stop.load(Ordering::SeqCst) {
                    break;
                }
            }
        });

        Ok(())
    }

    /// Send reload signal to all connected clients
    pub fn reload(&self) {
        let _ = self.reload_tx.send(());
    }

    /// Stop the server
    pub fn stop(&self) {
        self.should_stop.store(true, Ordering::SeqCst);
    }
}

async fn handle_client(stream: tokio::net::TcpStream, mut reload_rx: broadcast::Receiver<()>) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("WebSocket handshake error: {}", e);
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // Send initial connection message
    let _ = write
        .send(Message::Text(r#"{"type":"connected"}"#.to_string()))
        .await;

    loop {
        tokio::select! {
            // Handle reload signals
            _ = reload_rx.recv() => {
                let reload_msg = r#"{"type":"reload"}"#;
                if let Err(e) = write.send(Message::Text(reload_msg.to_string())).await {
                    eprintln!("WebSocket send error: {}", e);
                    break;
                }
            }
            // Handle client messages
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) => break,
                    Some(Err(e)) => {
                        eprintln!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}
