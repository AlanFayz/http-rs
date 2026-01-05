mod http;
mod router;
mod server;

use std::env;
use std::path::Path;
use std::pin::Pin;

use tokio::fs;

use crate::http::*;

use crate::router::*;
use crate::server::*;

async fn get_file_bytes(path: &str) -> tokio::io::Result<Vec<u8>> {
    let contents = fs::read(path).await?;
    Ok(contents)
}

fn is_safe_path(user_path: &str) -> bool {
    let path = Path::new(user_path);

    for component in path.components() {
        match component {
            std::path::Component::Normal(_) => continue,
            std::path::Component::CurDir => continue,
            _ => return false,
        }
    }
    true
}

fn global_route(request: HttpRequest) -> Pin<Box<dyn Future<Output = HttpResponse> + Send>> {
    return Box::pin(async move {
        let stripped_path = {
            if let Some(p) = request.path.strip_prefix("/") {
                p
            } else {
                request.path.as_str()
            }
        };

        if !is_safe_path(stripped_path) {
            return HttpResponse::new("HTTP/1.1", 401, "BAD PATH");
        }

        let contents = get_file_bytes(stripped_path).await;

        if let Err(_) = contents {
            return HttpResponse::new("HTTP/1.1", 401, "BAD");
        }
        let contents = contents.unwrap();

        let mut response = HttpResponse::new("HTTP/1.1", 200, "OK");
        response.insert_header("Content-Length", &contents.len().to_string());
        response.set_body(&contents);

        return response;
    });
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 2 && args.len() != 0 {
        println!("Usage: ./http ip port or ./http");
    }

    let (port, ip) = {
        if args.len() == 2 {
            (args[0].parse::<u16>()?, args[1].as_str())
        } else {
            (7878, "127.0.0.1")
        }
    };

    let server = Server::new(port, ip);

    let mut router = Router::new();
    router.get("*", Box::new(global_route));

    server.run(router).await?;
    return Ok(());
}
