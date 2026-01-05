use std::collections::HashMap;

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
    pub fn parse(data: Vec<String>) -> Result<HttpRequest, String> {
        if data.is_empty() {
            return Err("data was empty".to_owned());
        }

        let request_line = data[0].split(' ').collect::<Vec<_>>();
        if request_line.len() != 3 {
            return Err("invalid request".to_owned());
        }

        let method = HttpMethod::from(request_line[0]).ok_or("invalid method".to_owned())?;

        let uri = request_line[1].split('?').collect::<Vec<_>>();
        if uri.len() > 2 || uri.len() == 0 {
            return Err(format!("Invalid uri {}", request_line[1]));
        }

        let path = uri[0].to_string();
        let query_params = if uri.len() == 2 {
            parse_query_params(uri[1]).ok_or("invalid query params")?
        } else {
            HashMap::default()
        };

        let version = request_line[2].to_owned();
        let mut headers = HashMap::<String, String>::default();

        let mut i = 1;
        loop {
            if i >= data.len() || data[i].len() == 0 {
                break;
            }

            let header_seperator = data[i].find(':').ok_or("invalid header".to_owned())?;
            let (key, value) = data[i]
                .split_at_checked(header_seperator + 1)
                .ok_or("invalid header".to_owned())?;

            let mut key = key.to_owned();
            key.pop().ok_or("invalid header".to_owned())?;

            headers.insert(key.trim().to_string(), value.to_owned().trim().to_string());

            i += 1;
        }

        i += 1;

        let mut body: Vec<u8> = Vec::new();
        loop {
            if i >= data.len() || data[i].len() == 0 {
                break;
            }

            let mut bytes = data[i]
                .as_bytes()
                .iter()
                .map(|byte| *byte)
                .collect::<Vec<_>>();

            body.append(&mut bytes);

            i += 1;
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
    use super::*;

    #[test]
    fn test_http_request_parse_simple_get() {
        let input = vec![
            "GET /index.html HTTP/1.1".to_string(),
            "Host: 127.0.0.1:7878".to_string(),
            "Connection: keep-alive".to_string(),
            "".to_string(),
        ];

        let result = HttpRequest::parse(input).expect("Should successfully parse GET");

        assert!(matches!(result.method, HttpMethod::Get));
        assert_eq!(result.path, "/index.html");
        assert_eq!(result.version, "HTTP/1.1");
        assert_eq!(result.headers.get("Host").unwrap(), "127.0.0.1:7878");
        assert!(result.body.is_empty());
    }

    #[test]
    fn test_http_request_parse_post_with_body() {
        let input = vec![
            "POST /api/save HTTP/1.1".to_string(),
            "Content-Type: text/plain".to_string(),
            "Content-Length: 11".to_string(),
            "".to_string(),
            "hello world".to_string(),
        ];

        let result = HttpRequest::parse(input).expect("Should successfully parse POST");

        assert!(matches!(result.method, HttpMethod::Post));
        assert_eq!(result.path, "/api/save");
        assert_eq!(result.headers.get("Content-Type").unwrap(), "text/plain");

        assert_eq!(result.body, b"hello world");
        assert_eq!(String::from_utf8_lossy(&result.body), "hello world");
    }

    #[test]
    fn test_http_request_parse_invalid_first_line() {
        let input = vec!["NOT_A_METHOD /index HTTP/1.1".to_string()];
        let result = HttpRequest::parse(input);

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_browser_get_request() {
        let input = vec![
            "GET / HTTP/1.1".to_string(),
            "Host: 127.0.0.1:7878".to_string(),
            "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:99.0) Gecko/20100101 Firefox/99.0".to_string(),
            "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8".to_string(),
            "Accept-Language: en-US,en;q=0.5".to_string(),
            "Accept-Encoding: gzip, deflate, br".to_string(),
            "DNT: 1".to_string(),
            "Connection: keep-alive".to_string(),
            "Upgrade-Insecure-Requests: 1".to_string(),
            "Sec-Fetch-Dest: document".to_string(),
            "Sec-Fetch-Mode: navigate".to_string(),
            "Sec-Fetch-Site: none".to_string(),
            "Sec-Fetch-User: ?1".to_string(),
            "Cache-Control: max-age=0".to_string(),
        ];

        let result = HttpRequest::parse(input).expect("Failed to parse browser GET request");

        assert!(matches!(result.method, HttpMethod::Get));
        assert_eq!(result.path, "/");
        assert_eq!(result.version, "HTTP/1.1");

        assert_eq!(result.headers.get("Host").unwrap(), "127.0.0.1:7878");
        assert_eq!(result.headers.get("DNT").unwrap(), "1");
        assert_eq!(result.headers.get("Sec-Fetch-Mode").unwrap(), "navigate");

        assert!(result.body.is_empty());
    }

    #[test]
    fn test_parse_complex_query_params() {
        let input = vec![
            "GET /search?query=rust&verbose&mode= HTTP/1.1".to_string(),
            "Host: localhost".to_string(),
            "".to_string(),
        ];

        let result = HttpRequest::parse(input).expect("Failed to parse complex queries");

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
        assert!(
            response_str.contains(&expected_header),
            "Response should include correct Content-Length header"
        );
    }
}
