use std::{convert::Infallible, net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::{
    body::Incoming as Body, server::conn::http1 as server_http1, upgrade::Upgraded, Method,
    Request, Response, StatusCode,
};
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::{io, net::{TcpListener, TcpStream}};

use crate::shared_state::SharedState;

// Response body type alias
type ResponseBody = BoxBody<Bytes, hyper::Error>;

pub async fn run_proxy(state: Arc<SharedState>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8100));
    let listener = TcpListener::bind(addr).await?;
    println!("🚀 Listening on http://{}", addr);
    
    // Add startup log
    state.append_log(format!("🚀 Proxy server started on http://{}", addr));

    loop {
        let (stream, _) = listener.accept().await?;
        let state_for_conn = Arc::clone(&state);

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            
            // Create a separate clone for error logging
            let state_for_error = Arc::clone(&state_for_conn);

            let service = service_fn(move |req| {
                let state_for_req = Arc::clone(&state_for_conn);
                async move {
                    proxy(req, state_for_req).await
                }
            });

            if let Err(err) = server_http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, service)
                .with_upgrades()
                .await
            {
                eprintln!("❌ Connection error: {:?}", err);
                state_for_error.append_log(format!("❌ Connection error: {:?}", err));
            }
        });
    }
}

async fn proxy(
    req: Request<Body>,
    state: Arc<SharedState>,
) -> Result<Response<ResponseBody>, Infallible> {
    // Extract host for logging and store locally
    let host = req.uri().host().unwrap_or("unknown-host").to_string();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let is_connect = method == Method::CONNECT;
    
    if state.is_logging_enabled() {
        let log_entry = format!("{} {} {}", method, host, path);
        state.append_log(log_entry);
    }

    if !state.is_proxy_enabled() {
        if is_connect {
            if let Some(authority) = req.uri().authority() {
                let addr = authority.to_string();
                let req_clone = req;
    
                tokio::spawn(async move {
                    match hyper::upgrade::on(req_clone).await {
                        Ok(upgraded) => {
                            if let Err(e) = tunnel(upgraded, addr).await {
                                eprintln!("❌ Tunnel error (disabled proxy pass-through): {}", e);
                            }
                        }
                        Err(e) => eprintln!("❌ Upgrade error (disabled proxy pass-through): {}", e),
                    }
                });
    
                return Ok(Response::new(empty()));
            } else {
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(full("CONNECT must be to a socket address"))
                    .unwrap());
            }
        } else {
            // When proxy is disabled, return a service unavailable response
            state.record_request(&host, false); // Record as allowed since it's policy, not blocking
            return Ok(Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .body(full("🔌 Proxy is currently disabled — request blocked"))
                .unwrap());
        }
    }    

    // Check for tracker blocking for HTTP requests
    let is_blocked = match state.blocker.lock() {
        Ok(blocker) => {
            println!("Checking host: {}", host);
            blocker.is_blocked(&host)
        },
        Err(e) => {
            eprintln!("Failed to lock blocker: {:?}", e);
            state.append_log(format!("⚠️ Failed to check blocker: {:?}", e));
            false // Allow by default on error
        }
    };

    if is_blocked {
        // Record the blocked request in stats
        state.record_request(&host, true);
        
        // Log blocked request
        state.append_log(format!("🚫 Blocked request to tracker: {}", host));
        
        return Ok(Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(full(format!("🚫 Blocked request to tracker: {}", host)))
            .unwrap());
    }

    // Handle CONNECT method (for HTTPS tunneling)
    if is_connect {
        if let Some(authority) = req.uri().authority() {
            let addr = authority.to_string();
            let req_clone = req;
            let state_for_spawn = Arc::clone(&state);

            // Record the allowed request in stats
            state.record_request(&host, false);

            tokio::spawn(async move {
                match hyper::upgrade::on(req_clone).await {
                    Ok(upgraded) => {
                        if let Err(e) = tunnel(upgraded, addr.clone()).await {
                            eprintln!("❌ Tunnel error: {}", e);
                            state_for_spawn.append_log(format!("❌ Tunnel error with {}: {}", addr, e));
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Upgrade error: {}", e);
                        state_for_spawn.append_log(format!("❌ Upgrade error with {}: {}", addr, e));
                    }
                }
            });

            return Ok(Response::new(empty()));
        } else {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(full("CONNECT must be to a socket address"))
                .unwrap());
        }
    }

    // Normal HTTP forwarding
    // Record the allowed request in stats
    state.record_request(&host, false);
    
    let port = req.uri().port_u16().unwrap_or(80);
    let addr = format!("{}:{}", host, port);

    match TcpStream::connect(addr).await {
        Ok(stream) => {
            let io = TokioIo::new(stream);
            let (mut sender, conn) = match hyper::client::conn::http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .handshake(io)
                .await
            {
                Ok(parts) => parts,
                Err(e) => {
                    state.append_log(format!("❌ Handshake failed with {}: {:?}", host, e));
                    return Ok(Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .body(full("Handshake failed"))
                        .unwrap())
                }
            };

            tokio::spawn(async move {
                if let Err(err) = conn.await {
                    eprintln!("Connection failed: {:?}", err);
                }
            });

            match sender.send_request(req).await {
                Ok(resp) => Ok(resp.map(|b| b.boxed())),
                Err(e) => {
                    state.append_log(format!("❌ Request failed with {}: {:?}", host, e));
                    Ok(Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .body(full("Bad Gateway"))
                        .unwrap())
                }
            }
        }
        Err(e) => {
            state.append_log(format!("❌ Failed to connect to {}: {:?}", host, e));
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(full("Failed to connect to target host"))
                .unwrap())
        }
    }
}

// Response helpers
fn empty() -> ResponseBody {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

fn full<T: Into<Bytes>>(chunk: T) -> ResponseBody {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
    let mut server = TcpStream::connect(addr).await?;
    let mut upgraded = TokioIo::new(upgraded);
    let (from_client, from_server) = io::copy_bidirectional(&mut upgraded, &mut server).await?;
    println!(
        "🔒 Tunnel closed: client sent {} bytes, server sent {} bytes",
        from_client, from_server
    );
    Ok(())
}