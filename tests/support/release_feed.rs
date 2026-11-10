//! 通常の HTTPS リダイレクトと配布物だけを返す、外部へ接続しないテスト用プロキシ。
use anyhow::{Context, Result, ensure};
use rustls::{ServerConfig, ServerConnection, StreamOwned, pki_types::PrivatePkcs8KeyDer};
use skilvella::update::release::regression::Inputs;
use std::{
    fs,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const RELEASES: &str = "/1llum1n4t1s/Skilvella/releases";
const HOST: &str = "github.com";

pub struct ReleaseFeed {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl ReleaseFeed {
    pub fn start(cert_path: &Path, files: Arc<Inputs>) -> Result<Self> {
        let cert = rcgen::generate_simple_self_signed(vec![HOST.to_owned()])?;
        fs::write(cert_path, cert.cert.pem())?;
        let key = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert.cert.der().clone()], key.into())?;
        let config = Arc::new(config);
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let worker = thread::spawn(move || {
            while let Ok((stream, _)) = listener.accept() {
                if stopping.load(Ordering::Acquire) {
                    break;
                }
                if let Err(error) = serve(stream, config.clone(), &files) {
                    eprintln!("Release fixture: {error:#}");
                }
            }
        });
        Ok(Self {
            address,
            stop,
            worker: Some(worker),
        })
    }

    pub fn proxy_url(&self) -> String {
        format!("http://{}", self.address)
    }
}

impl Drop for ReleaseFeed {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_secs(1));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn first_line(mut input: impl Read) -> Result<String> {
    // CONNECT の直後の TLS データを先読みしない。
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        ensure!(header.len() < 8192, "Fixture request headers are too large");
        let mut byte = [0];
        input.read_exact(&mut byte)?;
        header.push(byte[0]);
    }
    Ok(std::str::from_utf8(&header)?
        .lines()
        .next()
        .context("Missing request line")?
        .to_owned())
}

fn serve(mut stream: TcpStream, config: Arc<ServerConfig>, files: &Inputs) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let connect = first_line(&mut stream)?;
    ensure!(
        connect == "CONNECT github.com:443 HTTP/1.1",
        "Unexpected proxy request: {connect}"
    );
    stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")?;
    let mut tls = StreamOwned::new(ServerConnection::new(config)?, stream);
    let request = first_line(&mut tls)?;
    let mut parts = request.split_whitespace();
    let method = parts.next().context("Missing method")?;
    let path = parts.next().context("Missing path")?;
    let version = files.candidate;
    let tag = format!("{RELEASES}/tag/v{version}");
    let latest = format!("{RELEASES}/latest");
    let download = format!("{RELEASES}/download/v{version}/");
    let (status, body, extra) = match (method, path) {
        ("HEAD", path) if path == latest => (
            "302 Found",
            &[][..],
            format!("Location: https://{HOST}{tag}\r\n"),
        ),
        ("HEAD", path) if path == tag => ("200 OK", &[][..], String::new()),
        ("GET", path) if path.strip_prefix(&download) == Some("SHA256SUMS.txt") => {
            ("200 OK", files.sums.as_slice(), String::new())
        }
        ("GET", path) if path.strip_prefix(&download) == Some(files.asset) => {
            ("200 OK", files.archive.as_slice(), String::new())
        }
        _ => {
            eprintln!("Unexpected release fixture request: {request}");
            ("404 Not Found", &[][..], String::new())
        }
    };
    println!("Fixture: {method} {path} -> {status}");
    write!(
        tls,
        "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n{extra}\r\n",
        body.len()
    )?;
    tls.write_all(body)?;
    tls.conn.send_close_notify();
    tls.flush()?;
    Ok(())
}
