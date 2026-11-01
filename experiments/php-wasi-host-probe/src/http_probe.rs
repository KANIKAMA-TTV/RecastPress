//! Closed loopback HTTP fixtures; no product listener or arbitrary PHP routing.
use super::{RequestInput, cgi_response, run_http};
use anyhow::{Context, Result, ensure};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderName, HeaderValue, Method, StatusCode, header},
    response::Response,
};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use tokio::{runtime, sync::oneshot, task::JoinHandle};
use wasmtime::{Engine, Module};

const MAX_REQUEST_BODY: usize = 4096;
const MAX_HTTP_RESPONSE: usize = 64 * 1024;
const PARTIAL_TIMEOUT: Duration = Duration::from_millis(750);
const FAILURE_BODY: &[u8] = b"PHP fixture execution failed\n";
const URI: &str = "/probe?term=a%2Bb&lang=ja%2FJP&empty=&flag";
const QUERY: &str = "term=a%2Bb&lang=ja%2FJP&empty=&flag";
const INPUT: &[u8] = b"http\0\xff\r\nbody";

struct AppState {
    engine: Engine,
    module: Module,
    next_nonce: AtomicU64,
    output_ready: mpsc::Sender<u64>,
    workers: Mutex<Vec<JoinHandle<()>>>,
    stopping: AtomicBool,
    partial_finished: Arc<AtomicBool>,
}

fn fixed_response(status: StatusCode, body: &'static [u8]) -> Response {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

fn completed_response(response: cgi_response::Response) -> Result<Response> {
    let mut builder = Response::builder().status(StatusCode::from_u16(response.status)?);
    for (name, value) in response.headers {
        builder = builder.header(
            HeaderName::from_bytes(name.as_bytes())?,
            HeaderValue::from_bytes(&value)?,
        );
    }
    // The 304 representation length cannot be validated here; deliberately omit
    // it. Hyper frames these known completed bytes, never the CGI length field.
    builder
        .body(Body::from(response.body))
        .context("cannot construct completed fixture response")
}

async fn handle(State(state): State<Arc<AppState>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let (method, mode, timeout) = match parts.uri.path() {
        "/probe" => (Method::POST, "basic", Duration::from_secs(5)),
        "/metadata" => (Method::GET, "response_metadata", Duration::from_secs(5)),
        "/partial" => (Method::GET, "http_partial", PARTIAL_TIMEOUT),
        _ => return fixed_response(StatusCode::NOT_FOUND, b"Unknown fixture\n"),
    };
    if parts.method != method {
        let mut response = fixed_response(
            StatusCode::METHOD_NOT_ALLOWED,
            b"Unsupported fixture method\n",
        );
        response.headers_mut().insert(
            header::ALLOW,
            HeaderValue::from_str(method.as_str()).expect("fixed fixture method"),
        );
        return response;
    }
    let body = match tokio::time::timeout(Duration::from_secs(2), to_bytes(body, MAX_REQUEST_BODY))
        .await
    {
        Ok(Ok(body)) => body,
        _ => return fixed_response(StatusCode::BAD_REQUEST, b"Invalid fixture body\n"),
    };
    let header_text = |name| -> Option<String> {
        match parts.headers.get(name) {
            Some(value) => value.to_str().ok().map(str::to_owned),
            None => Some(String::new()),
        }
    };
    let (Some(cookie), Some(content_type)) = (
        header_text(header::COOKIE),
        header_text(header::CONTENT_TYPE),
    ) else {
        return fixed_response(StatusCode::BAD_REQUEST, b"Invalid fixture headers\n");
    };
    let nonce = state.next_nonce.fetch_add(1, Ordering::Relaxed);
    let input = RequestInput {
        method: parts.method.as_str().to_owned(),
        uri: parts.uri.to_string(),
        query: parts.uri.query().unwrap_or_default().to_owned(),
        cookie,
        content_type,
        body: body.to_vec(),
        output_ready: (mode == "http_partial").then(|| state.output_ready.clone()),
    };
    let (sender, receiver) = oneshot::channel();
    {
        // Keep ownership of every blocking task, even if its HTTP connection
        // disappears. The lock also closes the spawn/drain race during cleanup.
        let mut workers = state.workers.lock().unwrap();
        if state.stopping.load(Ordering::Acquire) {
            return fixed_response(StatusCode::SERVICE_UNAVAILABLE, FAILURE_BODY);
        }
        let engine = state.engine.clone();
        let module = state.module.clone();
        let partial_finished = state.partial_finished.clone();
        workers.push(tokio::task::spawn_blocking(move || {
            let result = run_http(&engine, &module, mode, nonce, timeout, input)
                .and_then(|run| run.completed_response());
            if mode == "http_partial" {
                partial_finished.store(true, Ordering::Release);
            }
            let _ = sender.send(result);
        }));
    }
    match receiver.await {
        Ok(Ok(response)) => completed_response(response)
            .unwrap_or_else(|_| fixed_response(StatusCode::SERVICE_UNAVAILABLE, FAILURE_BODY)),
        _ => fixed_response(StatusCode::SERVICE_UNAVAILABLE, FAILURE_BODY),
    }
}

struct WireResponse {
    status: u16,
    headers: Vec<(String, Vec<u8>)>,
    body: Vec<u8>,
}

fn connect(address: SocketAddr, method: &str, uri: &str, body: &[u8]) -> Result<TcpStream> {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let headers = format!(
        "{method} {uri} HTTP/1.1\r\nHost: rp.invalid\r\nConnection: close\r\nContent-Type: application/octet-stream\r\nCookie: rp_fixture=41001; other=retained\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(stream)
}

fn read_response(stream: &mut TcpStream) -> Result<WireResponse> {
    read_response_with_prefix(stream, Vec::new())
}

fn read_response_with_prefix(stream: &mut TcpStream, mut wire: Vec<u8>) -> Result<WireResponse> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let remaining = (MAX_HTTP_RESPONSE + 1).saturating_sub(wire.len());
    stream.take(remaining as u64).read_to_end(&mut wire)?;
    ensure!(
        wire.len() <= MAX_HTTP_RESPONSE,
        "HTTP response exceeded fixture limit"
    );
    let separator = wire
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .context("missing HTTP response headers")?;
    let header_text = std::str::from_utf8(&wire[..separator])?;
    let mut lines = header_text.split("\r\n");
    let status_line = lines.next().context("missing HTTP status")?;
    let mut status = status_line.splitn(3, ' ');
    ensure!(status.next() == Some("HTTP/1.1"), "unexpected HTTP version");
    let status = status.next().context("missing HTTP status code")?.parse()?;
    let mut headers = Vec::new();
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .context("invalid HTTP response header")?;
        headers.push((name.to_ascii_lowercase(), value.trim().as_bytes().to_vec()));
    }
    let body = wire[separator + 4..].to_vec();
    let lengths: Vec<_> = headers
        .iter()
        .filter(|(name, _)| name == "content-length")
        .collect();
    ensure!(
        lengths.len() == 1,
        "missing or duplicate HTTP Content-Length"
    );
    let length: usize = std::str::from_utf8(&lengths[0].1)?.parse()?;
    ensure!(length == body.len(), "HTTP body framing mismatch");
    Ok(WireResponse {
        status,
        headers,
        body,
    })
}

fn check_normal(address: SocketAddr) -> Result<()> {
    let response = read_response(&mut connect(address, "POST", URI, INPUT)?)?;
    ensure!(response.status == 200, "normal HTTP fixture failed");
    let value: serde_json::Value = serde_json::from_slice(&response.body)?;
    ensure!(
        value["input_hex"] == "6874747000ff0d0a626f6479",
        "HTTP binary input changed"
    );
    ensure!(value["cookie"] == "41001", "HTTP Cookie did not reach PHP");
    ensure!(
        value["request_method"] == "POST",
        "HTTP method did not reach PHP"
    );
    ensure!(
        value["request_uri"] == URI && value["query_string"] == QUERY,
        "HTTP URI/query encoding changed"
    );
    ensure!(
        value["wp_hooks_initially_empty"] == true && value["cache_before"].is_null(),
        "HTTP request state leaked"
    );
    ensure!(
        value["wp_filter"] == "post|plugin|theme",
        "WordPress hook failed through HTTP"
    );
    Ok(())
}

fn check_cases(
    address: SocketAddr,
    output: &mpsc::Receiver<u64>,
    partial_finished: &AtomicBool,
) -> Result<()> {
    check_normal(address)?;
    let response = read_response(&mut connect(address, "GET", "/metadata", b"")?)?;
    ensure!(
        response.status == 201 && response.body == b"ok\0\xffbody",
        "HTTP status/binary output changed"
    );
    let cookies: Vec<_> = response
        .headers
        .iter()
        .filter(|(name, _)| name == "set-cookie")
        .map(|(_, value)| value.as_slice())
        .collect();
    ensure!(
        cookies
            == [
                b"first=1; Path=/; HttpOnly".as_slice(),
                b"second=2; Path=/; SameSite=Lax".as_slice()
            ],
        "HTTP Set-Cookie fields merged or changed"
    );
    ensure!(
        response
            .headers
            .iter()
            .any(|(name, value)| name == "content-type" && value == b"application/octet-stream"),
        "HTTP content type changed"
    );

    let mut partial = connect(address, "GET", "/partial", b"")?;
    output
        .recv_timeout(Duration::from_secs(3))
        .context("PHP did not flush the partial fixture output")?;
    ensure!(
        !partial_finished.load(Ordering::Acquire),
        "partial PHP request ended before pending inspection"
    );
    partial.set_read_timeout(Some(Duration::from_millis(50)))?;
    let mut byte = [0];
    let first_read = partial.read(&mut byte);
    ensure!(
        matches!(first_read, Err(ref error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)),
        "HTTP emitted bytes before PHP completed"
    );
    // A different request succeeds while the first PHP request is still pending.
    check_normal(address)?;
    ensure!(
        !partial_finished.load(Ordering::Acquire),
        "normal HTTP request did not finish while its sibling remained pending"
    );
    // Keep inspecting the whole pending interval, including the period after
    // the initial 50ms sample and the sibling's successful HTTP request.
    let inspection_deadline = Instant::now() + Duration::from_secs(3);
    let mut prefix = Vec::new();
    partial.set_read_timeout(Some(Duration::from_millis(20)))?;
    while !partial_finished.load(Ordering::Acquire) {
        ensure!(
            Instant::now() < inspection_deadline,
            "partial PHP request did not finish"
        );
        match partial.read(&mut byte) {
            Ok(count) => {
                ensure!(
                    partial_finished.load(Ordering::Acquire),
                    "HTTP emitted data or closed before PHP completed"
                );
                prefix.extend_from_slice(&byte[..count]);
                break;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
    let response = read_response_with_prefix(&mut partial, prefix)?;
    ensure!(
        partial_finished.load(Ordering::Acquire),
        "HTTP response preceded PHP worker completion"
    );
    ensure!(
        response.status == 503 && response.body == FAILURE_BODY,
        "failed PHP output leaked into HTTP"
    );
    check_normal(address).context("normal HTTP request after sibling timeout")?;
    Ok(())
}

pub fn verify(engine: &Engine, module: &Module) -> Result<()> {
    // Keep the default blocking pool: synchronous WASI filesystem operations can
    // require additional blocking jobs beyond the PHP worker itself.
    let runtime = runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let (output_ready, output) = mpsc::channel();
    let state = Arc::new(AppState {
        engine: engine.clone(),
        module: module.clone(),
        next_nonce: AtomicU64::new(40_001),
        output_ready,
        workers: Mutex::new(Vec::new()),
        stopping: AtomicBool::new(false),
        partial_finished: Arc::new(AtomicBool::new(false)),
    });
    let listener = runtime.block_on(tokio::net::TcpListener::bind((
        std::net::Ipv4Addr::LOCALHOST,
        0,
    )))?;
    let address = listener.local_addr()?;
    ensure!(
        address.ip().is_loopback(),
        "HTTP fixture listener is not loopback"
    );
    let (shutdown, stopped) = oneshot::channel();
    let router = Router::new().fallback(handle).with_state(state.clone());
    let mut server = runtime.spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
    });
    let result = check_cases(address, &output, &state.partial_finished);
    state.stopping.store(true, Ordering::Release);
    let _ = shutdown.send(());
    let cleanup = runtime.block_on(async {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
        let server_result = tokio::time::timeout_at(deadline, &mut server).await;
        let mut errors = Vec::new();
        match server_result {
            Ok(Ok(Ok(()))) => {}
            Ok(_) => errors.push("HTTP server failed during shutdown"),
            Err(_) => {
                server.abort();
                let _ = tokio::time::timeout(Duration::from_secs(1), &mut server).await;
                errors.push("HTTP server shutdown timed out");
            }
        }
        let workers = std::mem::take(&mut *state.workers.lock().unwrap());
        for mut worker in workers {
            if !matches!(
                tokio::time::timeout_at(deadline, &mut worker).await,
                Ok(Ok(()))
            ) {
                errors.push("HTTP PHP worker did not finish successfully");
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    });
    runtime.shutdown_timeout(Duration::from_secs(8));
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("HTTP fixture cleanup failed: {cleanup:#}")))
        }
    }
}
