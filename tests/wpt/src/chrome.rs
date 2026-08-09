use std::fs;
use std::io::Cursor;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail, ensure};
use thirtyfour::common::capabilities::chromium::ChromiumLikeCapabilities;
use thirtyfour::manager::BrowserKind;
use thirtyfour::{DesiredCapabilities, WebDriver};
use tiny_http::{Header, Method, Response, Server, StatusCode};
use tokio::runtime::{Builder, Runtime};
use url::Url;
use vello_cpu::Pixmap;

use crate::{VIEWPORT_HEIGHT, VIEWPORT_WIDTH};

const REFERENCE_READY_TIMEOUT_MS: usize = 10_000;
const AHEM_FONT_ROUTE: &str = "__gummy__/Ahem.ttf";
const AHEM_OVERRIDE_SCRIPT: &str = r#"
    const namespace = document.documentElement.namespaceURI;
    const style = namespace === 'http://www.w3.org/1999/xhtml'
        ? document.createElement('style')
        : document.createElementNS(namespace, 'style');
    style.textContent = `
        @font-face {
            font-family: "Gummy Ahem";
            src: url("/__gummy__/Ahem.ttf") format("truetype");
        }
        @layer gummy-ahem-override {
            *, *::before, *::after {
                font-family: "Gummy Ahem" !important;
                font-style: normal !important;
                font-weight: 400 !important;
                font-synthesis: none !important;
            }
        }
    `;
    const parent = document.head || document.documentElement;
    parent.insertBefore(style, parent.firstChild);
"#;

pub struct ChromeReferenceRenderer {
    state: Mutex<ChromeSession>,
    _server: WptHttpServer,
}

impl ChromeReferenceRenderer {
    pub fn start(wpt_dir: &Path, ahem_font: &Path) -> Result<Self> {
        let server = WptHttpServer::start(wpt_dir, ahem_font)?;
        let runtime = Builder::new_current_thread().enable_all().build().context("failed to create Tokio runtime")?;
        let client = runtime.block_on(connect_to_chrome()).context("failed to start a managed ChromeDriver session")?;
        if let Err(error) = runtime.block_on(configure_viewport(&client)) {
            let _ = runtime.block_on(client.quit());
            return Err(error).context("failed to configure Chrome's reference-test viewport");
        }

        Ok(Self { state: Mutex::new(ChromeSession { client: Some(client), runtime }), _server: server })
    }

    pub fn screenshot(&self, path: &Path) -> Result<Vec<u8>> {
        let url = self._server.url_for(path)?;
        let state = self.state.lock().map_err(|_| anyhow!("Chrome reference renderer lock was poisoned"))?;
        let client = state.client.as_ref().ok_or_else(|| anyhow!("Chrome reference session is closed"))?;
        let png = state
            .runtime
            .block_on(capture_reference(client, url.as_str()))
            .with_context(|| format!("failed to capture Chrome screenshot of {}", path.display()))?;
        screenshot_rgba(&png, path)
    }
}

impl Drop for ChromeReferenceRenderer {
    fn drop(&mut self) {
        let Ok(state) = self.state.get_mut() else {
            return;
        };
        if let Some(client) = state.client.take() {
            let _ = state.runtime.block_on(client.quit());
        }
    }
}

struct ChromeSession {
    client: Option<WebDriver>,
    runtime: Runtime,
}

async fn connect_to_chrome() -> Result<WebDriver> {
    let mut capabilities = DesiredCapabilities::chrome();
    for argument in [
        "--headless=new",
        "--disable-background-networking",
        "--disable-lcd-text",
        "--enable-blink-features=NoFontAntialiasing",
        "--force-color-profile=srgb",
        "--force-device-scale-factor=1",
        "--no-first-run",
        &format!("--window-size={VIEWPORT_WIDTH},{VIEWPORT_HEIGHT}"),
    ] {
        capabilities.add_arg(argument)?;
    }
    Ok(WebDriver::managed(capabilities)
        .driver_binary(BrowserKind::Chrome, "chromedriver")
        .ready_timeout(Duration::from_secs(10))
        .await?)
}

async fn configure_viewport(client: &WebDriver) -> Result<()> {
    client.goto("data:text/html,<title>viewport setup</title>").await?;

    for _ in 0..4 {
        let metrics = viewport_metrics(client).await?;
        ensure!(
            (metrics.device_pixel_ratio - 1.0).abs() < f64::EPSILON,
            "Chrome devicePixelRatio is {}; expected 1",
            metrics.device_pixel_ratio
        );
        if metrics.width == VIEWPORT_WIDTH as u64 && metrics.height == VIEWPORT_HEIGHT as u64 {
            return Ok(());
        }

        let outer = client.get_window_rect().await?;
        let target_outer_width = adjusted_outer_dimension(outer.width, metrics.width, VIEWPORT_WIDTH)?;
        let target_outer_height = adjusted_outer_dimension(outer.height, metrics.height, VIEWPORT_HEIGHT)?;
        client.set_window_rect(outer.x, outer.y, target_outer_width, target_outer_height).await?;
    }

    let metrics = viewport_metrics(client).await?;
    bail!(
        "Chrome viewport is {}x{} after resizing; expected {}x{}",
        metrics.width,
        metrics.height,
        VIEWPORT_WIDTH,
        VIEWPORT_HEIGHT
    )
}

fn adjusted_outer_dimension(outer: i64, inner: u64, target: usize) -> Result<u32> {
    let adjusted = outer as i128 + target as i128 - inner as i128;
    u32::try_from(adjusted).context("Chrome reported an invalid window dimension")
}

#[derive(Clone, Copy, Debug)]
struct ViewportMetrics {
    width: u64,
    height: u64,
    device_pixel_ratio: f64,
}

async fn viewport_metrics(client: &WebDriver) -> Result<ViewportMetrics> {
    let (width, height, device_pixel_ratio) = client
        .execute("return [window.innerWidth, window.innerHeight, window.devicePixelRatio]", Vec::new())
        .await?
        .convert()?;
    Ok(ViewportMetrics { width, height, device_pixel_ratio })
}

async fn capture_reference(client: &WebDriver, url: &str) -> Result<Vec<u8>> {
    client.goto(url).await?;
    client.execute(AHEM_OVERRIDE_SCRIPT, Vec::new()).await?;
    let readiness_script = r#"
                const done = arguments[arguments.length - 1];
                const root = document.documentElement;
                const waitForReftest = new Promise((resolve, reject) => {
                    if (!root.classList.contains('reftest-wait')) {
                        resolve();
                        return;
                    }
                    const observer = new MutationObserver(() => {
                        if (!root.classList.contains('reftest-wait')) {
                            observer.disconnect();
                            resolve();
                        }
                    });
                    observer.observe(root, { attributes: true, attributeFilter: ['class'] });
                    setTimeout(() => {
                        observer.disconnect();
                        reject(new Error('timed out waiting for reftest-wait to be removed'));
                    }, REFERENCE_READY_TIMEOUT_MS);
                });
                waitForReftest
                    .then(() => document.fonts ? document.fonts.load('16px "Gummy Ahem"') : undefined)
                    .then(() => document.fonts ? document.fonts.ready : undefined)
                    .then(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))
                    .then(() => done(null), error => done(String(error)));
            "#
    .replace("REFERENCE_READY_TIMEOUT_MS", &REFERENCE_READY_TIMEOUT_MS.to_string());
    let ready: Option<String> = client.execute_async(&readiness_script, Vec::new()).await?.convert()?;
    if let Some(error) = ready {
        bail!("Chrome reference did not become ready: {error}");
    }
    Ok(client.screenshot_as_png().await?)
}

fn screenshot_rgba(png: &[u8], path: &Path) -> Result<Vec<u8>> {
    let pixmap = Pixmap::from_png(Cursor::new(png))
        .with_context(|| format!("Chrome returned an invalid PNG screenshot for {}", path.display()))?;
    ensure!(
        pixmap.width() as usize == VIEWPORT_WIDTH && pixmap.height() as usize == VIEWPORT_HEIGHT,
        "Chrome screenshot of {} is {}x{}; expected {}x{}",
        path.display(),
        pixmap.width(),
        pixmap.height(),
        VIEWPORT_WIDTH,
        VIEWPORT_HEIGHT
    );
    Ok(pixmap.data_as_u8_slice().to_vec())
}

struct WptHttpServer {
    root: PathBuf,
    address: SocketAddr,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WptHttpServer {
    fn start(root: &Path, ahem_font: &Path) -> Result<Self> {
        let root =
            fs::canonicalize(root).with_context(|| format!("failed to resolve WPT directory {}", root.display()))?;
        let ahem_font = fs::canonicalize(ahem_font)
            .with_context(|| format!("failed to resolve Ahem font {}", ahem_font.display()))?;
        let server =
            Server::http(("127.0.0.1", 0)).map_err(|error| anyhow!("failed to start WPT HTTP server: {error}"))?;
        let address =
            server.server_addr().to_ip().ok_or_else(|| anyhow!("WPT HTTP server did not bind to an IP address"))?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = shutdown.clone();
        let thread_root = root.clone();
        let thread_ahem_font = ahem_font.clone();
        let thread = thread::spawn(move || serve_wpt(server, &thread_root, &thread_ahem_font, &thread_shutdown));
        Ok(Self { root, address, shutdown, thread: Some(thread) })
    }

    fn url_for(&self, path: &Path) -> Result<Url> {
        let path = fs::canonicalize(path).with_context(|| format!("failed to resolve reference {}", path.display()))?;
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return Url::from_file_path(&path)
                .map_err(|()| anyhow!("failed to create a file URL for reference {}", path.display()));
        };

        let mut url = Url::parse(&format!("http://{}/", self.address))?;
        let mut segments = url.path_segments_mut().map_err(|()| anyhow!("failed to build WPT reference URL"))?;
        for component in relative.components() {
            let Component::Normal(component) = component else {
                continue;
            };
            segments.push(
                component.to_str().ok_or_else(|| anyhow!("reference path is not valid UTF-8: {}", path.display()))?,
            );
        }
        drop(segments);
        Ok(url)
    }
}

impl Drop for WptHttpServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve_wpt(server: Server, root: &Path, ahem_font: &Path, shutdown: &AtomicBool) {
    while !shutdown.load(Ordering::Relaxed) {
        match server.recv_timeout(Duration::from_millis(100)) {
            Ok(Some(request)) => respond_with_file(request, root, ahem_font),
            Ok(None) => {}
            Err(_) => break,
        }
    }
}

fn respond_with_file(request: tiny_http::Request, root: &Path, ahem_font: &Path) {
    let Some(relative) = request_path(request.url()) else {
        let _ = request.respond(Response::empty(StatusCode(400)));
        return;
    };
    if relative == Path::new(AHEM_FONT_ROUTE) {
        respond_with_path(request, ahem_font);
        return;
    }
    let path = root.join(relative);
    let Ok(path) = fs::canonicalize(path) else {
        let _ = request.respond(Response::empty(StatusCode(404)));
        return;
    };
    if !path.starts_with(root) || !path.is_file() {
        let _ = request.respond(Response::empty(StatusCode(404)));
        return;
    }

    respond_with_path(request, &path);
}

fn respond_with_path(request: tiny_http::Request, path: &Path) {
    let content_type = Header::from_bytes("Content-Type", mime_type(&path)).expect("static content type is valid");
    if request.method() == &Method::Head {
        let _ = request.respond(Response::empty(StatusCode(200)).with_header(content_type));
        return;
    }
    let response = match fs::File::open(&path) {
        Ok(file) => Response::from_file(file).with_header(content_type),
        Err(_) => {
            let _ = request.respond(Response::empty(StatusCode(404)));
            return;
        }
    };
    let _ = request.respond(response);
}

fn request_path(url: &str) -> Option<PathBuf> {
    let path = url.split_once('?').map_or(url, |(path, _)| path).trim_start_matches('/');
    let decoded = percent_decode(path)?;
    let relative = Path::new(&decoded);
    if relative.components().all(|component| matches!(component, Component::Normal(_) | Component::CurDir)) {
        Some(relative.to_path_buf())
    } else {
        None
    }
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_digit(*bytes.get(index + 1)?)?;
            let low = hex_digit(*bytes.get(index + 2)?)?;
            decoded.push(high << 4 | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()).unwrap_or_default().to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "xhtml" | "xht" => "application/xhtml+xml",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_paths_are_decoded_and_confined() {
        assert_eq!(request_path("/css/a%20b/ref.html?ignored=true"), Some(PathBuf::from("css/a b/ref.html")));
        assert_eq!(request_path("/../secret"), None);
        assert_eq!(request_path("/%2e%2e/secret"), None);
        assert_eq!(request_path("/%invalid"), None);
    }
}
