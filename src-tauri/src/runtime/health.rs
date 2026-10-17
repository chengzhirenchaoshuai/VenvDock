//! 健康检查：向服务网址发一个最简单的 HTTP 请求，判断服务是否还在响应
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// 连接和读写的超时
const TIMEOUT: Duration = Duration::from_secs(5);

/// 服务有响应时返回 true：http 要求收到状态码小于 500 的回复；https 不做握手，只检查端口能否连上
pub fn probe(url: &str) -> bool {
    let (https, rest) = if let Some(rest) = url.strip_prefix("http://") {
        (false, rest)
    } else if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else {
        return false;
    };
    let rest = rest.split('#').next().unwrap_or_default();
    let (authority, path) = match rest.find(['/', '?']) {
        Some(i) if rest[i..].starts_with('/') => (&rest[..i], rest[i..].to_string()),
        Some(i) => (&rest[..i], format!("/{}", &rest[i..])),
        None => (rest, "/".to_string()),
    };
    // 冒号在最后一个 ] 之后才是端口（IPv6 地址形如 [::1]:8080）
    let has_port = authority
        .rfind(':')
        .is_some_and(|i| authority.rfind(']').is_none_or(|b| i > b));
    let addr = if has_port {
        authority.to_string()
    } else {
        format!("{authority}:{}", if https { 443 } else { 80 })
    };
    let Some(mut stream) = connect(&addr) else {
        return false;
    };
    if https {
        return true;
    }
    let _ = stream.set_read_timeout(Some(TIMEOUT));
    let _ = stream.set_write_timeout(Some(TIMEOUT));
    let request = format!("GET {path} HTTP/1.0\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    // 只需要状态行，例如 “HTTP/1.1 200 OK”
    let mut buf = [0u8; 32];
    let mut len = 0;
    while len < buf.len() {
        match stream.read(&mut buf[len..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => len += n,
        }
    }
    status_code(&buf[..len]).is_some_and(|code| code < 500)
}

/// 依次尝试解析出的地址（localhost 可能同时对应 ::1 和 127.0.0.1）
fn connect(addr: &str) -> Option<TcpStream> {
    addr.to_socket_addrs()
        .ok()?
        .find_map(|a| TcpStream::connect_timeout(&a, TIMEOUT).ok())
}

fn status_code(head: &[u8]) -> Option<u16> {
    let text = std::str::from_utf8(head).ok()?;
    let mut parts = text.split_whitespace();
    parts.next().filter(|v| v.starts_with("HTTP/"))?;
    parts.next()?.parse().ok()
}
