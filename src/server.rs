use std::sync::Arc;

use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

use crate::{http::*, router::*};

pub struct Server {
    port: u16,
    ip: String,
}

impl Server {
    pub fn new(port: u16, host: &str) -> Server {
        Server {
            port,
            ip: host.to_owned(),
        }
    }

    pub async fn run<T: Send + Sync + 'static>(
        &self,
        router: Router<T>,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

    async fn handle_connection<T>(
        socket: TcpStream,
        router: &Arc<Router<T>>,
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
