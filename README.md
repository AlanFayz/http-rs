# http-rs

A custom, asynchronous HTTP server and routing engine built from scratch in Rust. 

## Usage Modes

### 1. As a Library
Integrate the router into your own application logic using the fluent API.

```rust
let mut router = Router::new();
router.get("/user/:id", Box::new(user_handler));
server.run(router).await?;
```

### 2. Standalone HTTP Server
By leveraging the greedy wildcard (*) and the path-sanitization logic, http-rs can function as a standalone static file server. Simply point the global route to a file-retrieval handler to serve a directory over HTTP. This allows the binary to act as a replacement for tools like python -m http.server.

```bash
cargo run -- 8080 0.0.0.0
cargo run 
```

## Tests 
Unit tests are included for modules, simply do

```
cargo test
```