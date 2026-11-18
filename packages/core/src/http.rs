//! Small helpers for the launcher's authenticated calls to its own server.

/// The host of `host[:port]` or `[v6]:port`, without brackets.
pub fn host_of(authority: &str) -> &str {
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    if let Some(v6) = authority.strip_prefix('[') {
        return v6.split(']').next().unwrap_or("");
    }
    authority.split(':').next().unwrap_or("")
}

/// Whether `host` is this machine: `localhost`, 127.0.0.0/8 or ::1.
pub fn loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.to_canonical().is_loopback())
}

/// Whether a Bearer token may be sent to `server`: any https server, or a plain http one on loopback.
pub fn bearer_allowed(server: &str) -> bool {
    match server.strip_prefix("http://") {
        Some(rest) => loopback(host_of(rest.split('/').next().unwrap_or(""))),
        None => true,
    }
}

/// One field of a `multipart/form-data` body.
pub struct Field<'a> {
    pub name: &'a str,
    pub file_name: Option<&'a str>,
    pub content_type: &'a str,
    pub data: &'a [u8],
}

/// The `Content-Type` header and the body for `fields`, with a boundary no field contains.
pub fn multipart(fields: &[Field]) -> (String, Vec<u8>) {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut n = 0u32;
    let boundary = loop {
        let b = format!("swofty-{seed:x}-{}-{n}", std::process::id());
        if !fields.iter().any(|f| contains(f.data, b.as_bytes())) {
            break b;
        }
        n += 1;
    };
    let mut body = Vec::new();
    for f in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"",
                f.name
            )
            .as_bytes(),
        );
        if let Some(file) = f.file_name {
            let file = file.replace(['"', '\r', '\n'], "_");
            body.extend_from_slice(format!("; filename=\"{file}\"").as_bytes());
        }
        body.extend_from_slice(format!("\r\nContent-Type: {}\r\n\r\n", f.content_type).as_bytes());
        body.extend_from_slice(f.data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_host_of_an_authority() {
        assert_eq!(host_of("[::1]:8471"), "::1");
        assert_eq!(host_of("user@127.0.0.1:80"), "127.0.0.1");
    }

    #[test]
    fn builds_a_multipart_body_with_a_free_boundary() {
        let (ct, body) = multipart(&[
            Field {
                name: "meta",
                file_name: None,
                content_type: "application/json",
                data: b"{}",
            },
            Field {
                name: "lap",
                file_name: Some("a\"b.swlap"),
                content_type: "application/octet-stream",
                data: b"SWLAP",
            },
        ]);
        let boundary = ct.strip_prefix("multipart/form-data; boundary=").unwrap();
        let text = String::from_utf8(body).unwrap();
        assert!(text.starts_with(&format!("--{boundary}\r\n")));
        assert!(text.ends_with(&format!("--{boundary}--\r\n")));
        assert!(text.contains("name=\"meta\"\r\nContent-Type: application/json\r\n\r\n{}\r\n"));
        assert!(text.contains("name=\"lap\"; filename=\"a_b.swlap\""));
    }

    #[test]
    fn sends_a_token_over_https_or_loopback_only() {
        assert!(bearer_allowed("https://f1.swofty.net"));
        assert!(bearer_allowed("http://127.0.0.1:8471"));
        assert!(bearer_allowed("http://localhost:1"));
        assert!(!bearer_allowed("http://10.0.0.5:8471"));
        assert!(!bearer_allowed("http://127.0.0.1.example.com"));
    }
}
