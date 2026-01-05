use std::{collections::HashMap, fmt, pin::Pin};

use crate::http::{HttpMethod, HttpRequest, HttpResponse};

pub type Handler =
    Box<dyn Fn(HttpRequest) -> Pin<Box<dyn Future<Output = HttpResponse> + Send>> + Send + Sync>;

#[derive(PartialEq, Hash, Clone, Debug)]
enum RouterItem {
    Static(String),
    Param(String),
    Wildcard,
}

struct RouterNode {
    pub handlers: HashMap<HttpMethod, Handler>,
    next: HashMap<RouterItem, RouterNode>,
}

pub struct Router {
    root_node: RouterNode,
}

macro_rules! generate_http_methods {
    ($( $x:ident => $y:expr ),*) => {
        $(
            pub fn $x(&mut self, path: &str, f: Handler) -> &mut Self {
                self.insert_route($y, path, f);
                return self;
            }
        )*
    };
}

impl Eq for RouterItem {}

impl fmt::Debug for RouterNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouterNode")
            .field("next", &self.next)
            .field("handlers_count", &self.handlers.len())
            .finish()
    }
}

impl RouterNode {
    fn new() -> RouterNode {
        RouterNode {
            handlers: HashMap::default(),
            next: HashMap::default(),
        }
    }

    fn lookup(&self, id: &str) -> Option<&RouterNode> {
        self.next
            .get(&RouterItem::Static(id.to_string()))
            .or_else(|| {
                let id = id.strip_prefix(':').map(|x| x.to_string())?;
                self.next.get(&RouterItem::Param(id))
            })
            .or_else(|| self.next.get(&RouterItem::Wildcard))
    }

    fn insert_handler(&mut self, method: HttpMethod, mut path: std::str::Split<char>, f: Handler) {
        let current_segment = match path.next() {
            Some(s) => s,
            None => {
                self.handlers.insert(method, f);
                return;
            }
        };

        let item = {
            if let Some(param) = current_segment.strip_prefix(":") {
                RouterItem::Param(param.to_string())
            } else if current_segment == "*" {
                RouterItem::Wildcard
            } else {
                RouterItem::Static(current_segment.to_string())
            }
        };

        if !self.next.contains_key(&item) {
            self.next.insert(item.clone(), RouterNode::new());
        }

        let node = self.next.get_mut(&item).unwrap();

        if item == RouterItem::Wildcard {
            node.handlers.insert(method, f);
        } else {
            node.insert_handler(method, path, f);
        }
    }

    fn get_handler(
        &self,
        req: &mut HttpRequest,
        mut path: std::str::Split<char>,
    ) -> Option<&Handler> {
        let current_segment = match path.next() {
            Some(s) => s,
            None => return self.handlers.get(&req.method),
        };

        if let Some(node) = self.lookup(current_segment) {
            if let Some(handler) = node.get_handler(req, path.clone()) {
                return Some(handler);
            }
        }

        for (item, node) in self.next.iter() {
            if let RouterItem::Param(param_name) = item {
                if let Some(handler) = node.get_handler(req, path.clone()) {
                    req.params
                        .insert(param_name.to_string(), current_segment.to_string());

                    return Some(handler);
                }
            }
        }

        for (item, node) in self.next.iter() {
            if let RouterItem::Wildcard = item {
                if let Some(handler) = node.handlers.get(&req.method) {
                    req.params
                        .insert("*".to_string(), path.collect::<Vec<_>>().join("/"));

                    return Some(handler);
                }
            }
        }

        None
    }
}

impl Router {
    pub fn new() -> Router {
        Router {
            root_node: RouterNode::new(),
        }
    }

    fn insert_route(&mut self, method: HttpMethod, path: &str, f: Handler) {
        let path = path.split('/');
        self.root_node.insert_handler(method, path, f);
    }

    generate_http_methods!(
        get => HttpMethod::Get,
        head => HttpMethod::Head,
        post => HttpMethod::Post,
        put => HttpMethod::Put,
        delete => HttpMethod::Delete,
        connect => HttpMethod::Connect,
        options => HttpMethod::Options,
        trace => HttpMethod::Trace,
        patch => HttpMethod::Patch
    );

    pub async fn fetch(&self, mut request: HttpRequest) -> Option<HttpResponse> {
        let path = request.path.clone();
        let route = self.root_node.get_handler(&mut request, path.split('/'))?;
        return Some(route(request).await);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_req(method: HttpMethod, path: &str) -> HttpRequest {
        HttpRequest {
            method,
            path: path.to_string(),
            ..Default::default()
        }
    }

    fn mock_handler(body: &'static str) -> Handler {
        Box::new(move |_req| {
            Box::pin(async move { HttpResponse::body(body.to_string().as_bytes().to_vec(), None) })
        })
    }

    #[tokio::test]
    async fn test_basic_routing() {
        let mut router = Router::new();
        router.get("/hello/world", mock_handler("static_match"));

        let req = make_req(HttpMethod::Get, "/hello/world");
        let res = router.fetch(req).await.unwrap();
        assert_eq!(res.body, b"static_match");

        let req_fail = make_req(HttpMethod::Get, "/not/found");
        assert!(router.fetch(req_fail).await.is_none());
    }

    #[tokio::test]
    async fn test_parameter_matching() {
        let mut router = Router::new();
        router.get("/user/:id", mock_handler("user_profile"));
        router.get("/user/:id/settings", mock_handler("user_settings"));

        let req = make_req(HttpMethod::Get, "/user/123");
        let res = router.fetch(req.clone()).await.unwrap();

        assert_eq!(res.body, b"user_profile");

        router.get("/user/admin", mock_handler("admin_panel"));
        let req_admin = make_req(HttpMethod::Get, "/user/admin");
        let res_admin = router.fetch(req_admin).await.unwrap();
        assert_eq!(res_admin.body, b"admin_panel");
    }

    #[tokio::test]
    async fn test_wildcard_greedy_matching() {
        let mut router = Router::new();
        router.get("/static/*", mock_handler("static_file"));

        let req = make_req(HttpMethod::Get, "/static/css/theme/dark.css");
        let res = router.fetch(req).await.unwrap();
        assert_eq!(res.body, b"static_file");

        router.get("/*", mock_handler("fallback"));
        let req_fb = make_req(HttpMethod::Get, "/some/random/path");
        let res_fb = router.fetch(req_fb).await.unwrap();
        assert_eq!(res_fb.body, b"fallback");
    }

    #[tokio::test]
    async fn test_parameter_extraction_logic() {
        let mut router = Router::new();

        router.get(
            "/blog/:post_id/comment/:comment_id",
            Box::new(|req| {
                Box::pin(async move {
                    let p_id = req.params.get("post_id").unwrap();
                    let c_id = req.params.get("comment_id").unwrap();
                    HttpResponse::body(format!("{}:{}", p_id, c_id).as_bytes().to_vec(), None)
                })
            }),
        );

        let req = make_req(HttpMethod::Get, "/blog/my-first-post/comment/42");
        let res = router.fetch(req).await.unwrap();
        assert_eq!(res.body, b"my-first-post:42");
    }
}
