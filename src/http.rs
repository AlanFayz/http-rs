use std::collections::HashMap;

use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    net::TcpStream,
};

#[derive(Default, Debug, PartialEq, Hash, Clone, Copy)]
pub enum HttpMethod {
    #[default]
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
    Patch,
}

#[derive(Default, Debug, Clone)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub query_params: HashMap<String, Option<String>>,
    pub params: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Default, Debug, Clone)]
pub struct HttpResponse {
    version: String,
    status_code: u16,
    status_text: String,
    headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

fn parse_query_params(params: &str) -> Option<HashMap<String, Option<String>>> {
    let params = params
        .split('&')
        .map(|s| s.split('=').collect::<Vec<_>>())
        .collect::<Vec<_>>();

    let mut query_params_map: HashMap<String, Option<String>> = HashMap::default();
    for param in &params {
        if param.len() == 0 || param.len() > 2 {
            return None;
        }

        let value = if param.len() == 2 {
            Some(param[1].trim().to_owned())
        } else {
            None
        };

        query_params_map.insert(param[0].trim().to_owned(), value);
    }

    return Some(query_params_map);
}

impl Eq for HttpMethod {}

impl HttpResponse {
    pub fn new(version: &str, status_code: u16, status_text: &str) -> HttpResponse {
        HttpResponse {
            version: version.to_string(),
            status_code,
            status_text: status_text.to_string(),
            headers: HashMap::default(),
            body: Vec::default(),
        }
    }

    pub fn insert_header(&mut self, key: &str, value: &str) {
        self.headers.insert(key.to_string(), value.to_string());
    }

    pub fn set_body(&mut self, body: &[u8]) {
        self.body = body.to_vec();
    }

    pub fn get_bytes(&mut self) -> Vec<u8> {
        let status_line = format!("{} {} {}", self.version, self.status_code, self.status_text);
        let length = self.body.len();

        let mut response = format!("{status_line}\r\nContent-Length: {length}\r\n");
        for (key, value) in &self.headers {
            response += format!("{}: {}\r\n", key, value).as_str();
        }

        response += "\r\n";
        let mut response = response
            .as_bytes()
            .iter()
            .map(|byte| *byte)
            .collect::<Vec<_>>();

        response.append(&mut self.body);
        return response;
    }
}

impl HttpMethod {
    pub fn from(s: &str) -> Option<HttpMethod> {
        match s {
            "GET" => Some(HttpMethod::Get),
            "HEAD" => Some(HttpMethod::Head),
            "POST" => Some(HttpMethod::Post),
            "PUT" => Some(HttpMethod::Put),
            "DELETE" => Some(HttpMethod::Delete),
            "CONNECT" => Some(HttpMethod::Connect),
            "OPTIONS" => Some(HttpMethod::Options),
            "TRACE" => Some(HttpMethod::Trace),
            "PATCH" => Some(HttpMethod::Patch),
            _ => None,
        }
    }
}

impl HttpRequest {
    pub async fn parse<R: AsyncRead + Unpin>(
        reader: &mut BufReader<R>,
    ) -> Result<HttpRequest, Box<dyn std::error::Error>> {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;

        if n == 0 {
            return Ok(HttpRequest::default());
        }

        let request_line = line.trim().split(' ').collect::<Vec<_>>();
        if request_line.len() != 3 {
            return Err("request line must be made up of 3 components".into());
        }

        let method = HttpMethod::from(request_line[0]).ok_or("invalid method".to_owned())?;

        let uri = request_line[1].split('?').collect::<Vec<_>>();
        if uri.len() > 2 || uri.len() == 0 {
            return Err(format!("Invalid uri {}", request_line[1]).into());
        }

        let path = uri[0].to_string();
        let query_params = if uri.len() == 2 {
            parse_query_params(uri[1]).ok_or("invalid query params")?
        } else {
            HashMap::default()
        };

        let version = request_line[2].to_owned();
        let mut headers = HashMap::<String, String>::default();

        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).await?;

            if n == 0 {
                return Ok(HttpRequest::default());
            }

            let line = line.trim();
            if line.is_empty() {
                break;
            }

            let header_seperator = line.find(':').ok_or("invalid header".to_owned())?;
            let (key, value) = line
                .split_at_checked(header_seperator + 1)
                .ok_or("invalid header".to_owned())?;

            let mut key = key.to_owned();
            key.pop().ok_or("invalid header".to_owned())?;

            headers.insert(key.trim().to_string(), value.to_owned().trim().to_string());
        }

        let mut body: Vec<u8> = Vec::new();

        if let Some(content_length) = headers.get("Content-Length") {
            let content_length: usize = content_length.parse()?;

            let mut line = String::new();
            reader.read_line(&mut line).await?;

            loop {
                if body.len() >= content_length {
                    break;
                }

                let mut line = String::new();
                let n = reader.read_line(&mut line).await?;

                if n == 0 {
                    break;
                }

                let mut bytes = line.as_bytes().iter().map(|byte| *byte).collect::<Vec<_>>();
                body.append(&mut bytes);
            }
        }

        return Ok(HttpRequest {
            method,
            path,
            version,
            headers,
            query_params,
            params: HashMap::default(),
            body,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[tokio::test]
    async fn test_http_request_parse_simple_get() {
        let input = vec![
            "GET /index.html HTTP/1.1",
            "Host: 127.0.0.1:7878",
            "Connection: keep-alive",
            "",
            "",
        ]
        .join("\r\n");

        let mut reader = BufReader::new(std::io::Cursor::new(input));

        let result = HttpRequest::parse(&mut reader)
            .await
            .expect("Should successfully parse GET");

        assert!(matches!(result.method, HttpMethod::Get));
        assert_eq!(result.path, "/index.html");
        assert_eq!(result.version, "HTTP/1.1");
        assert_eq!(result.headers.get("Host").unwrap(), "127.0.0.1:7878");
        assert!(result.body.is_empty());
    }

    #[tokio::test]
    async fn test_http_request_parse_post_with_body() {
        let input = vec![
            "POST /api/save HTTP/1.1",
            "Content-Type: text/plain",
            "Content-Length: 11",
            "",
            "",
            "hello world",
        ]
        .join("\r\n");

        let mut reader = BufReader::new(Cursor::new(input));
        let result = HttpRequest::parse(&mut reader)
            .await
            .expect("Should successfully parse POST");

        assert!(matches!(result.method, HttpMethod::Post));
        assert_eq!(result.path, "/api/save");
        assert_eq!(result.headers.get("Content-Type").unwrap(), "text/plain");
        assert_eq!(result.body, b"hello world");
    }

    #[tokio::test]
    async fn test_http_request_parse_invalid_first_line() {
        let input = "NOT_A_METHOD /index HTTP/1.1\r\n\r\n";
        let mut reader = BufReader::new(Cursor::new(input));

        let result = HttpRequest::parse(&mut reader).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_browser_get_request() {
        let input = vec![
            "GET / HTTP/1.1",
            "Host: 127.0.0.1:7878",
            "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:99.0) Gecko/20100101 Firefox/99.0",
            "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
            "Accept-Language: en-US,en;q=0.5",
            "Accept-Encoding: gzip, deflate, br",
            "DNT: 1",
            "Connection: keep-alive",
            "Upgrade-Insecure-Requests: 1",
            "Sec-Fetch-Dest: document",
            "Sec-Fetch-Mode: navigate",
            "Sec-Fetch-Site: none",
            "Sec-Fetch-User: ?1",
            "Cache-Control: max-age=0",
            "",
            "",
        ]
        .join("\r\n");

        let mut reader = BufReader::new(Cursor::new(input));
        let result = HttpRequest::parse(&mut reader)
            .await
            .expect("Failed to parse browser GET request");

        assert!(matches!(result.method, HttpMethod::Get));
        assert_eq!(result.path, "/");
        assert_eq!(result.headers.get("Host").unwrap(), "127.0.0.1:7878");
        assert_eq!(result.headers.get("DNT").unwrap(), "1");
        assert_eq!(result.headers.get("Sec-Fetch-Mode").unwrap(), "navigate");
    }

    #[tokio::test]
    async fn test_parse_complex_query_params() {
        let input = vec![
            "GET /search?query=rust&verbose&mode= HTTP/1.1",
            "Host: localhost",
            "",
            "",
        ]
        .join("\r\n");

        let mut reader = BufReader::new(Cursor::new(input));
        let result = HttpRequest::parse(&mut reader)
            .await
            .expect("Failed to parse complex queries");

        assert_eq!(
            result.query_params.get("query").unwrap(),
            &Some("rust".to_string())
        );
        assert_eq!(result.query_params.get("verbose").unwrap(), &None);
        assert_eq!(
            result.query_params.get("mode").unwrap(),
            &Some("".to_string())
        );
        assert_eq!(result.query_params.len(), 3);
    }

    #[test]
    fn test_response_status_line_only() {
        let mut response = HttpResponse::new("HTTP/1.1", 204, "No Content");
        let bytes = response.get_bytes();
        let response_str = String::from_utf8_lossy(&bytes);

        assert!(response_str.starts_with("HTTP/1.1 204 No Content\r\n"));
        assert!(response_str.ends_with("\r\n\r\n"));
    }

    #[test]
    fn test_response_with_headers_and_body() {
        let mut response = HttpResponse::new("HTTP/1.1", 200, "OK");
        response.insert_header("Content-Type", "text/html");
        response.set_body(b"<html><body>Hello</body></html>");

        let bytes = response.get_bytes();
        let response_str = String::from_utf8_lossy(&bytes);

        assert!(response_str.contains("HTTP/1.1 200 OK\r\n"));
        assert!(response_str.contains("Content-Type: text/html\r\n"));
        assert!(response_str.contains("\r\n\r\n<html><body>Hello</body></html>"));
    }

    #[test]
    fn test_content_length_calculation() {
        let mut response = HttpResponse::new("HTTP/1.1", 200, "OK");
        let body_data = b"Rust Programming";
        response.set_body(body_data);

        let bytes = response.get_bytes();
        let response_str = String::from_utf8_lossy(&bytes);

        let expected_header = format!("Content-Length: {}", body_data.len());
        assert!(response_str.contains(&expected_header));
    }
}
