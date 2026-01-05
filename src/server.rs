use std::{sync::Arc, time::Duration};

use tokio::{
    fs::{self},
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, TcpStream},
    sync::Mutex,
    time::timeout,
};

use crate::{http::*, router::*};

pub struct Server {
    port: u16,
    ip: String,
}

async fn get_file_bytes(path: &str) -> tokio::io::Result<Vec<u8>> {
    let contents = fs::read(path).await?;
    Ok(contents)
}

impl Server {
    pub fn new(port: u16, host: &str) -> Server {
        Server {
            port,
            ip: host.to_owned(),
        }
    }

    pub async fn run(&self, router: Router) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.ip, self.port.to_string());
        let listener = TcpListener::bind(addr).await?;
        let router = Arc::new(router);

        loop {
            let (socket, _) = listener.accept().await?;
            let router_local = Arc::clone(&router);

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(socket, &router_local).await {
                    eprintln!("Error handling connection: {}", e);
                }
            });
        }
    }

    async fn handle_connection(
        socket: TcpStream,
        router: &Arc<Router>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut reader = BufReader::new(socket);

        let request = HttpRequest::parse(&mut reader).await?;
        let mut response = router
            .fetch(request)
            .await
            .unwrap_or(HttpResponse::not_found("route not found"));

        let mut socket = reader.into_inner();
        socket.write_all(&response.get_bytes()).await?;
        return Ok(());
    }
}
