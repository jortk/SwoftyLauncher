use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

use crate::bytes::Reader;
use crate::{Error, Result};
use md5::{Digest, Md5};

pub const MAGIC: &[u8; 4] = b"ERPK";
pub const HEADER_LEN: usize = 0x30;
pub const ZSTD: u8 = 0x11;
pub const STORED: u8 = 0x91;
pub const ZSTD_LEVEL: i32 = 19;
/// The window every frame is written with, whatever the level: 1 MiB, the
/// largest the game's own archives use. The game's stream decoder sizes its
/// buffer from a frame's window and content size, inside a fixed heap, so a
/// level-19 window (8 MiB) on every fragment runs that heap out.
pub const ZSTD_WINDOW_LOG: u32 = 20;
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
/// The waits between tries of an archive's open or replace that Windows
/// refused for now: an antivirus holds a file it scans for a moment.
const BACKOFF_MS: [u64; 6] = [25, 50, 100, 200, 400, 800];
/// The fewest index bytes a resource entry takes: its length, name length,
/// kind, the shorter type argument, fragment count and hash.
const MIN_ENTRY_LEN: usize = 4 + 2 + 16 + 4 + 1 + 16;

#[derive(Debug, Clone)]
pub struct Fragment {
    pub name: [u8; 4],
    pub offset: u64,
    pub size: u64,
    pub flags: u32,
    pub compression: u8,
    pub packed: u64,
    pub raw: Vec<u8>,
}

impl Fragment {
    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.name)
            .trim_end_matches('\0')
            .to_owned()
    }

    pub fn data(&self) -> Result<Vec<u8>> {
        self.data_cow().map(std::borrow::Cow::into_owned)
    }

    pub(crate) fn data_cow(&self) -> Result<std::borrow::Cow<'_, [u8]>> {
        if self.compression == ZSTD || self.compression != STORED && self.raw.starts_with(&ZSTD_MAGIC) {
            Ok(zstd::decode_all(self.raw.as_slice())?.into())
        } else {
            Ok(std::borrow::Cow::Borrowed(&self.raw))
        }
    }
}

#[derive(Debug, Clone)]
pub struct Resource {
    pub name: String,
    pub kind: [u8; 16],
    pub unk: Vec<u8>,
    pub fragments: Vec<Fragment>,
    pub hash: [u8; 16],
}

impl Resource {
    pub fn kind(&self) -> String {
        String::from_utf8_lossy(&self.kind)
            .trim_end_matches('\0')
            .to_owned()
    }

    pub fn content_hash(&self) -> Result<[u8; 16]> {
        let mut h = Md5::new();
        for f in &self.fragments {
            h.update(f.data()?);
        }
        Ok(h.finalize().into())
    }

    pub fn like(name: &str, template: &Resource, data: &[Vec<u8>]) -> Result<Self> {
        check_name(name)?;
        if data.len() != template.fragments.len() {
            return Err(Error::Format(format!(
                "{name}: {} fragment(s) given, a {} like {} has {}",
                data.len(),
                template.kind(),
                template.name,
                template.fragments.len()
            )));
        }
        let mut res = Resource {
            name: name.to_owned(),
            kind: template.kind,
            unk: template.unk.clone(),
            fragments: Vec::with_capacity(data.len()),
            hash: [0; 16],
        };
        for (t, d) in template.fragments.iter().zip(data) {
            let mut f = Fragment {
                name: t.name,
                offset: 0,
                size: 0,
                flags: t.flags,
                compression: t.compression,
                packed: 0,
                raw: Vec::new(),
            };
            fill(&mut f, d)?;
            res.fragments.push(f);
        }
        res.hash = res.content_hash()?;
        Ok(res)
    }

    pub fn has_stats(&self) -> bool {
        matches!(&self.fragments[..], [_, s, ..] if s.size == crate::bxml::STATS_LEN as u64)
    }

    pub fn stale_stats(&self) -> Result<bool> {
        match &self.fragments[..] {
            [data, stats, ..] if self.has_stats() => {
                Ok(crate::bxml::stale_stats(&data.data()?, &stats.data()?))
            }
            _ => Ok(false),
        }
    }

    // Rewrites stale TSMD stats from fragment 0, as the game's own tools write them;
    // true when it did. Fails when fragment 0 starts as binary XML but does not parse.
    pub fn repair_stats(&mut self) -> Result<bool> {
        if !self.stale_stats()? {
            return Ok(false);
        }
        let stats = self.fragments[1].data()?;
        let version = u32::from_le_bytes(stats[4..8].try_into().expect("4 bytes"));
        let doc = crate::bxml::Document::parse(&self.fragments[0].data()?)?;
        self.set_fragment(1, &doc.string_stats(version))?;
        Ok(true)
    }

    pub fn set_fragment(&mut self, index: usize, data: &[u8]) -> Result<()> {
        let f = self
            .fragments
            .get_mut(index)
            .ok_or_else(|| Error::Format(format!("{}: no fragment {index}", self.name)))?;
        fill(f, data)?;
        self.hash = self.content_hash()?;
        Ok(())
    }
}

fn fill(f: &mut Fragment, data: &[u8]) -> Result<()> {
    f.raw = if f.compression == ZSTD {
        compress(data)?
    } else {
        data.to_vec()
    };
    f.size = data.len() as u64;
    f.packed = f.raw.len() as u64;
    Ok(())
}

pub fn check_name(name: &str) -> Result<()> {
    if name.is_empty() || name.contains('\0') || name.len() >= u16::MAX as usize {
        return Err(Error::Format(format!(
            "{name:?} is not a resource name: it must be 1 to {} bytes without NUL",
            u16::MAX - 1
        )));
    }
    Ok(())
}

pub fn kind(name: &str) -> Result<[u8; 16]> {
    let b = name.as_bytes();
    if b.is_empty() || b.len() >= 16 || b.contains(&0) {
        return Err(Error::Format(format!(
            "{name:?} is not a resource type: it must be 1 to 15 bytes without NUL"
        )));
    }
    let mut out = [0; 16];
    out[..b.len()].copy_from_slice(b);
    Ok(out)
}

pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    compress_at(data, ZSTD_LEVEL)
}

pub fn compress_at(data: &[u8], level: i32) -> Result<Vec<u8>> {
    let dir = PACKED.lock().unwrap_or_else(|e| e.into_inner()).clone();
    match dir.filter(|_| data.len() >= PACKED_MIN) {
        Some(dir) => compress_cached(&dir, data, level),
        None => encode(data, level),
    }
}

static PACKED: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

static WORK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A count that grows as fragments are compressed and archives written, so a
/// caller can tell a long build that is working from one that is stuck.
pub fn work() -> u64 {
    WORK.load(std::sync::atomic::Ordering::Relaxed)
}

fn worked() {
    WORK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}
const PACKED_MIN: usize = 16 << 10;

pub fn cache_compressed(dir: Option<std::path::PathBuf>) {
    if dir.is_none() {
        ENCODERS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
    *PACKED.lock().unwrap_or_else(|e| e.into_inner()) = dir;
}

type Encoder = zstd::zstd_safe::CCtx<'static>;

/// Compression contexts kept between fragments, by level. Setting one up
/// starts a worker thread and allocates its window, which cost as much as
/// compressing a small fragment. A reused context keeps its parameters, so it
/// writes the same bytes as a new one.
static ENCODERS: std::sync::Mutex<Vec<(i32, Encoder)>> = std::sync::Mutex::new(Vec::new());

fn zstd_error(code: usize) -> Error {
    Error::Format(format!("zstd: {}", zstd::zstd_safe::get_error_name(code)))
}

/// The settings `zstd -c -<level> --zstd=wlog=20 --stream-size=<n>` compresses
/// with: a checksum, the content size, a 1 MiB window, and the command line's
/// multithreaded path with one worker, whose output differs from the
/// single-threaded encoder's on large inputs.
fn encoder(level: i32) -> Result<Encoder> {
    use zstd::zstd_safe::CParameter;
    let mut ctx =
        Encoder::try_create().ok_or_else(|| Error::Format("zstd: no memory for an encoder".into()))?;
    for p in [
        CParameter::CompressionLevel(level),
        CParameter::WindowLog(ZSTD_WINDOW_LOG),
        CParameter::ContentSizeFlag(true),
        CParameter::ChecksumFlag(true),
        CParameter::NbWorkers(1),
    ] {
        ctx.set_parameter(p).map_err(zstd_error)?;
    }
    Ok(ctx)
}

fn encode(data: &[u8], level: i32) -> Result<Vec<u8>> {
    use zstd::zstd_safe::{InBuffer, OutBuffer, ResetDirective, zstd_sys::ZSTD_EndDirective};
    worked();
    let pooled = {
        let mut pool = ENCODERS.lock().unwrap_or_else(|e| e.into_inner());
        pool.iter()
            .position(|(l, _)| *l == level)
            .map(|i| pool.swap_remove(i).1)
    };
    let mut ctx = match pooled {
        Some(ctx) => ctx,
        None => encoder(level)?,
    };
    ctx.reset(ResetDirective::SessionOnly).map_err(zstd_error)?;
    ctx.set_pledged_src_size(Some(data.len() as u64))
        .map_err(zstd_error)?;
    let mut out: Vec<u8> = Vec::with_capacity(zstd::compress_bound(data.len()) + 64);
    let mut input = InBuffer::around(data);
    // The input goes in first and the frame ends after, as the command line
    // does with a pledged size, so the bytes match the command line's.
    while input.pos() < data.len() {
        if out.capacity() - out.len() < (1 << 16) {
            out.reserve(1 << 16);
        }
        let at = out.len();
        ctx.compress_stream2(
            &mut OutBuffer::around_pos(&mut out, at),
            &mut input,
            ZSTD_EndDirective::ZSTD_e_continue,
        )
        .map_err(zstd_error)?;
    }
    let mut end = InBuffer::around(&[]);
    loop {
        let at = out.len();
        let left = ctx
            .compress_stream2(
                &mut OutBuffer::around_pos(&mut out, at),
                &mut end,
                ZSTD_EndDirective::ZSTD_e_end,
            )
            .map_err(zstd_error)?;
        if left == 0 {
            break;
        }
        out.reserve(left.max(1 << 16));
    }
    ENCODERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((level, ctx));
    Ok(out)
}

fn compress_cached(dir: &Path, data: &[u8], level: i32) -> Result<Vec<u8>> {
    let digest: String = Md5::digest(data).iter().map(|b| format!("{b:02x}")).collect();
    let name = format!(
        "{digest}-{}-{level}-w{ZSTD_WINDOW_LOG}-{}",
        data.len(),
        zstd::zstd_safe::version_number()
    );
    let path = dir.join(&name[..2]).join(&name);
    if let Ok(raw) = std::fs::read(&path)
        && zstd::decode_all(raw.as_slice()).is_ok_and(|back| back == data)
    {
        if let Ok(f) = std::fs::File::options().write(true).open(&path) {
            f.set_modified(std::time::SystemTime::now()).ok();
        }
        return Ok(raw);
    }
    let raw = encode(data, level)?;
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let stored = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&tmp, &raw))
        .and_then(|()| std::fs::rename(&tmp, &path));
    if stored.is_err() {
        std::fs::remove_file(&tmp).ok();
    }
    Ok(raw)
}

#[derive(Debug, Clone)]
pub struct Erp {
    pub header: [u8; HEADER_LEN],
    pub version: u32,
    pub resources: Vec<Resource>,
}

impl Erp {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let (mut erp, data_off) = Self::parse_index(data)?;
        for res in &mut erp.resources {
            for f in &mut res.fragments {
                f.raw = Reader::new(data, data_off.saturating_add(f.offset as usize))
                    .take(f.packed as usize)?
                    .to_vec();
            }
        }
        Ok(erp)
    }

    pub fn read_matching(path: &Path, want: impl Fn(&str) -> bool) -> Result<Vec<Resource>> {
        let file = ErpFile::open(path)?;
        file.erp
            .resources
            .iter()
            .filter(|r| want(&r.name))
            .map(|r| file.load(r))
            .collect()
    }

    fn parse_index(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = Reader::new(data, 0);
        let header: [u8; HEADER_LEN] = r.array()?;
        if &header[..4] != MAGIC {
            return Err(Error::Magic {
                offset: 0,
                expected: "ERPK",
            });
        }
        let version = u32::from_le_bytes(header[4..8].try_into().expect("4 bytes"));
        let data_off = u64::from_le_bytes(header[0x20..0x28].try_into().expect("8 bytes")) as usize;
        let count = r.u32()?;
        let _fragments = r.u32()?;
        let fit = data.len().saturating_sub(r.pos) / MIN_ENTRY_LEN;
        let mut resources = Vec::with_capacity((count as usize).min(fit));
        for _ in 0..count {
            let _entry_len = r.u32()?;
            let name_len = r.u16()? as usize;
            let name = String::from_utf8_lossy(r.take(name_len)?)
                .trim_end_matches('\0')
                .to_owned();
            let kind = r.array()?;
            let unk = r.take(if version >= 4 { 6 } else { 4 })?.to_vec();
            let n = r.u8()?;
            let mut fragments = Vec::with_capacity(n as usize);
            for _ in 0..n {
                fragments.push(Fragment {
                    name: r.array()?,
                    offset: r.u64()?,
                    size: r.u64()?,
                    flags: r.u32()?,
                    compression: r.u8()?,
                    packed: r.u64()?,
                    raw: Vec::new(),
                });
            }
            resources.push(Resource {
                name,
                kind,
                unk,
                fragments,
                hash: r.array()?,
            });
        }
        if r.pos != data_off {
            return Err(Error::Format(format!(
                "index ends at {:#x}, data starts at {data_off:#x}",
                r.pos
            )));
        }
        Ok((
            Self {
                header,
                version,
                resources,
            },
            data_off,
        ))
    }

    pub fn find(&self, name: &str) -> Result<&Resource> {
        let mut hits = self.resources.iter().filter(|r| r.name == name);
        match (hits.next(), hits.next()) {
            (Some(r), None) => Ok(r),
            (None, _) => Err(Error::Format(format!("{name}: not found"))),
            _ => Err(Error::Format(format!("{name}: more than one resource"))),
        }
    }

    pub fn find_mut(&mut self, name: &str) -> Result<&mut Resource> {
        self.find(name)?;
        Ok(self
            .resources
            .iter_mut()
            .find(|r| r.name == name)
            .expect("found above"))
    }

    pub fn to_bytes(&mut self) -> Vec<u8> {
        let mut out = self.layout();
        for res in &self.resources {
            for f in &res.fragments {
                out.extend_from_slice(&f.raw);
            }
        }
        out
    }

    fn layout(&mut self) -> Vec<u8> {
        let mut index = Vec::new();
        let mut fragments = 0u32;
        let mut offset = 0u64;
        for res in &mut self.resources {
            let mut e = Vec::new();
            let name_len = res.name.len() + 1;
            e.extend_from_slice(&(name_len as u16).to_le_bytes());
            e.extend_from_slice(res.name.as_bytes());
            e.push(0);
            e.extend_from_slice(&res.kind);
            e.extend_from_slice(&res.unk);
            e.push(res.fragments.len() as u8);
            for f in &mut res.fragments {
                f.offset = offset;
                offset += f.packed;
                e.extend_from_slice(&f.name);
                e.extend_from_slice(&f.offset.to_le_bytes());
                e.extend_from_slice(&f.size.to_le_bytes());
                e.extend_from_slice(&f.flags.to_le_bytes());
                e.push(f.compression);
                e.extend_from_slice(&f.packed.to_le_bytes());
                fragments += 1;
            }
            e.extend_from_slice(&res.hash);
            index.extend_from_slice(&(e.len() as u32).to_le_bytes());
            index.extend_from_slice(&e);
        }
        let info_len = 8 + index.len() as u64;
        let mut header = self.header;
        header[0x10..0x18].copy_from_slice(&(HEADER_LEN as u64).to_le_bytes());
        header[0x18..0x20].copy_from_slice(&info_len.to_le_bytes());
        header[0x20..0x28].copy_from_slice(&(HEADER_LEN as u64 + info_len).to_le_bytes());
        let mut out = Vec::with_capacity(HEADER_LEN + info_len as usize + offset as usize);
        out.extend_from_slice(&header);
        out.extend_from_slice(&(self.resources.len() as u32).to_le_bytes());
        out.extend_from_slice(&fragments.to_le_bytes());
        out.extend_from_slice(&index);
        out
    }
}

pub enum Entry<'a> {
    Copy(&'a Resource),
    Owned(Resource),
}

#[derive(Debug, Clone)]
pub struct ErpFile {
    pub path: std::path::PathBuf,
    pub erp: Erp,
    pub data_off: u64,
}

impl ErpFile {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = open(path)?;
        let mut index = vec![0; HEADER_LEN];
        file.read_exact(&mut index).map_err(crate::io_at(path))?;
        if &index[..4] != MAGIC {
            return Err(Error::Magic {
                offset: 0,
                expected: "ERPK",
            });
        }
        let data_off = u64::from_le_bytes(index[0x20..0x28].try_into().expect("8 bytes"));
        if data_off < HEADER_LEN as u64 {
            return Err(Error::Format(format!(
                "{}: data starts at {data_off:#x}, inside the {HEADER_LEN:#x}-byte header",
                path.display()
            )));
        }
        let len = file.metadata().map_err(crate::io_at(path))?.len();
        if data_off > len {
            return Err(crate::io_at(path)(std::io::ErrorKind::UnexpectedEof.into()));
        }
        index.resize(data_off as usize, 0);
        file.read_exact(&mut index[HEADER_LEN..])
            .map_err(crate::io_at(path))?;
        Ok(Self {
            path: path.to_path_buf(),
            erp: Erp::parse_index(&index)?.0,
            data_off,
        })
    }

    pub fn load(&self, res: &Resource) -> Result<Resource> {
        let mut file = open(&self.path)?;
        let len = file.metadata().map_err(crate::io_at(&self.path))?.len();
        let mut out = res.clone();
        for f in &mut out.fragments {
            let end = self
                .data_off
                .checked_add(f.offset)
                .and_then(|at| at.checked_add(f.packed));
            if end.is_none_or(|end| end > len) {
                return Err(crate::io_at(&self.path)(std::io::ErrorKind::UnexpectedEof.into()));
            }
            file.seek(SeekFrom::Start(self.data_off + f.offset))
                .map_err(crate::io_at(&self.path))?;
            f.raw = vec![0; f.packed as usize];
            file.read_exact(&mut f.raw).map_err(crate::io_at(&self.path))?;
        }
        Ok(out)
    }

    pub fn write(&self, entries: Vec<Entry<'_>>, dst: &Path) -> Result<()> {
        let mut spans = Vec::new();
        let mut resources = Vec::with_capacity(entries.len());
        for e in entries {
            match e {
                Entry::Copy(r) => {
                    let mut r = r.clone();
                    for f in &mut r.fragments {
                        spans.push(Some((self.data_off + f.offset, f.packed)));
                        f.raw = Vec::new();
                    }
                    resources.push(r);
                }
                Entry::Owned(r) => {
                    spans.extend(r.fragments.iter().map(|_| None));
                    resources.push(r);
                }
            }
        }
        let mut erp = Erp {
            header: self.erp.header,
            version: self.erp.version,
            resources,
        };
        let head = erp.layout();
        let tmp = dst.with_extension("erp.tmp");
        let mut src = open(&self.path)?;
        let mut out = std::io::BufWriter::with_capacity(
            1 << 20,
            retry(|| std::fs::File::create(&tmp)).map_err(crate::io_at(&tmp))?,
        );
        std::io::Write::write_all(&mut out, &head)?;
        let mut buf = vec![0; 1 << 20];
        let frags = erp.resources.iter().flat_map(|r| &r.fragments);
        for (f, span) in frags.zip(spans) {
            match span {
                None => std::io::Write::write_all(&mut out, &f.raw)?,
                Some((at, mut left)) => {
                    src.seek(SeekFrom::Start(at)).map_err(crate::io_at(&self.path))?;
                    while left > 0 {
                        let n = left.min(buf.len() as u64) as usize;
                        src.read_exact(&mut buf[..n]).map_err(crate::io_at(&self.path))?;
                        std::io::Write::write_all(&mut out, &buf[..n])?;
                        left -= n as u64;
                    }
                }
            }
        }
        std::io::Write::flush(&mut out)?;
        drop(out);
        drop(src);
        if let Err(e) = replace(&tmp, dst) {
            std::fs::remove_file(&tmp).ok();
            return Err(crate::io_at(dst)(e));
        }
        worked();
        Ok(())
    }
}

fn transient(e: &std::io::Error) -> bool {
    const ERROR_ACCESS_DENIED: i32 = 5;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    cfg!(windows)
        && matches!(
            e.raw_os_error(),
            Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION)
        )
}

fn retrying<T>(
    transient: &dyn Fn(&std::io::Error) -> bool,
    wait: &mut dyn FnMut(Duration),
    mut f: impl FnMut() -> std::io::Result<T>,
) -> std::io::Result<T> {
    let mut waits = BACKOFF_MS.iter();
    loop {
        match f() {
            Err(e) if transient(&e) => match waits.next() {
                Some(ms) => wait(Duration::from_millis(*ms)),
                None => return Err(e),
            },
            done => return done,
        }
    }
}

fn retry<T>(f: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    retrying(&transient, &mut std::thread::sleep, f)
}

fn open(path: &Path) -> Result<std::fs::File> {
    retry(|| std::fs::File::open(path)).map_err(crate::io_at(path))
}

/// Moves `tmp` over `dst`, clearing a read-only `dst` first, and tries again
/// for a moment while Windows refuses it, as it does while an antivirus scans
/// either file. Nothing of ours may hold `dst` open.
pub fn replace(tmp: &Path, dst: &Path) -> std::io::Result<()> {
    retry(|| {
        writable(dst);
        std::fs::rename(tmp, dst)
    })
}

#[cfg(windows)]
#[allow(clippy::permissions_set_readonly_false)]
fn writable(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path)
        && meta.permissions().readonly()
    {
        let mut perms = meta.permissions();
        perms.set_readonly(false);
        std::fs::set_permissions(path, perms).ok();
    }
}

#[cfg(not(windows))]
fn writable(_: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cut_short_archive_names_its_file() {
        let dir = std::env::temp_dir().join(format!("swofty-erp-short-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut head = vec![0; HEADER_LEN];
        head[..4].copy_from_slice(MAGIC);
        head[0x20..0x28].copy_from_slice(&(HEADER_LEN as u64 + 100).to_le_bytes());
        for (name, body) in [("index.erp", head.clone()), ("header.erp", head[..10].to_vec())] {
            let path = dir.join(name);
            std::fs::write(&path, body).unwrap();
            let err = ErpFile::open(&path).unwrap_err();
            assert!(
                matches!(&err, Error::IoAt { path: p, source } if p == &path && source.kind() == std::io::ErrorKind::UnexpectedEof),
                "{err}"
            );
            assert_eq!(
                err.to_string(),
                format!(
                    "{}: the file is shorter than it should be, so it is incomplete or damaged",
                    path.display()
                )
            );
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_bad_data_offset_or_count_is_an_error_not_a_panic() {
        let dir = std::env::temp_dir().join(format!("swofty-erp-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut head = vec![0; HEADER_LEN];
        head[..4].copy_from_slice(MAGIC);
        let path = dir.join("zero.erp");
        std::fs::write(&path, &head).unwrap();
        assert!(matches!(ErpFile::open(&path), Err(Error::Format(_))));

        head[0x20..0x28].copy_from_slice(&u64::MAX.to_le_bytes());
        let path = dir.join("huge.erp");
        std::fs::write(&path, &head).unwrap();
        let err = ErpFile::open(&path).unwrap_err();
        assert!(
            matches!(&err, Error::IoAt { path: p, source } if p == &path && source.kind() == std::io::ErrorKind::UnexpectedEof),
            "{err}"
        );

        let mut counted = head.clone();
        counted[0x20..0x28].copy_from_slice(&(HEADER_LEN as u64 + 8).to_le_bytes());
        counted.extend_from_slice(&u32::MAX.to_le_bytes());
        counted.extend_from_slice(&0u32.to_le_bytes());
        let path = dir.join("count.erp");
        std::fs::write(&path, &counted).unwrap();
        assert!(matches!(ErpFile::open(&path), Err(Error::Eof(_))));
        assert!(matches!(Erp::parse(&counted), Err(Error::Eof(_))));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_fragment_past_the_end_of_its_file_is_an_error_not_a_panic() {
        let path = std::env::temp_dir().join(format!("swofty-erp-packed-{}.erp", std::process::id()));
        std::fs::write(&path, sample().to_bytes()).unwrap();
        let file = ErpFile::open(&path).unwrap();
        let mut res = file.erp.resources[0].clone();
        res.fragments[0].packed = u64::MAX;
        let err = file.load(&res).unwrap_err();
        assert!(
            matches!(&err, Error::IoAt { source, .. } if source.kind() == std::io::ErrorKind::UnexpectedEof),
            "{err}"
        );
        res.fragments[0].packed = 1 << 40;
        assert!(file.load(&res).is_err());
        assert!(file.load(&file.erp.resources[0]).is_ok());
        std::fs::remove_file(&path).ok();
    }

    fn streamed(data: &[u8], level: i32) -> Vec<u8> {
        let mut enc = zstd::Encoder::new(Vec::new(), level).unwrap();
        enc.include_checksum(true).unwrap();
        enc.include_contentsize(true).unwrap();
        enc.window_log(ZSTD_WINDOW_LOG).unwrap();
        enc.set_pledged_src_size(Some(data.len() as u64)).unwrap();
        enc.multithread(1).unwrap();
        std::io::Write::write_all(&mut enc, data).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn a_reused_encoder_writes_what_a_new_one_does() {
        let mut seed = 7u64;
        let mut noisy = |n: usize| -> Vec<u8> {
            (0..n)
                .map(|i| {
                    seed = seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    if i % 3 == 0 {
                        (seed >> 59) as u8
                    } else {
                        b"track_asset_"[i % 12]
                    }
                })
                .collect()
        };
        let inputs = [
            Vec::new(),
            noisy(1),
            noisy(4 << 10),
            noisy(70 << 10),
            noisy(3 << 20),
            noisy(9 << 20),
        ];
        for level in [3, ZSTD_LEVEL] {
            for _ in 0..2 {
                for data in &inputs {
                    assert_eq!(
                        encode(data, level).unwrap(),
                        streamed(data, level),
                        "{} bytes, level {level}",
                        data.len()
                    );
                }
            }
        }
        cache_compressed(None);
        assert_eq!(
            encode(&inputs[2], ZSTD_LEVEL).unwrap(),
            streamed(&inputs[2], ZSTD_LEVEL)
        );
    }

    /// The content size and window a frame header declares (RFC 8878 3.1.1.1).
    fn frame_header(raw: &[u8]) -> (Option<u64>, u64) {
        assert_eq!(raw[..4], ZSTD_MAGIC);
        let fhd = raw[4];
        let single = fhd & 0x20 != 0;
        let mut at = 5;
        let mut window = 0;
        if !single {
            let (exponent, mantissa) = (u64::from(raw[at] >> 3), u64::from(raw[at] & 7));
            let base = 1u64 << (10 + exponent);
            window = base + base / 8 * mantissa;
            at += 1;
        }
        at += [0, 1, 2, 4][usize::from(fhd & 3)];
        let size = match (fhd >> 6, single) {
            (0, false) => None,
            (0, true) => Some(u64::from(raw[at])),
            (1, _) => Some(u64::from(u16::from_le_bytes([raw[at], raw[at + 1]])) + 256),
            (2, _) => Some(u64::from(u32::from_le_bytes(raw[at..at + 4].try_into().unwrap()))),
            _ => Some(u64::from_le_bytes(raw[at..at + 8].try_into().unwrap())),
        };
        (size, if single { size.unwrap() } else { window })
    }

    /// What zstd 1.5.6+'s stream decoder allocates for a frame: its input
    /// block buffer and its window ring, which a known content size caps.
    fn decoder_buffer(size: Option<u64>, window: u64) -> u64 {
        let block = window.min(128 << 10);
        let ring = window + 2 * block + 64;
        size.map_or(ring, |s| s.min(ring)) + block.max(4)
    }

    #[test]
    fn every_frame_carries_its_content_size_and_at_most_a_1_mib_window() {
        let noisy = |n: usize| -> Vec<u8> {
            let mut seed = n as u64 | 1;
            (0..n)
                .map(|i| {
                    seed = seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    if i % 4 == 0 {
                        (seed >> 56) as u8
                    } else {
                        b"livery_"[i % 7]
                    }
                })
                .collect()
        };
        for level in [3, ZSTD_LEVEL] {
            for n in [0, 1, 300, 70 << 10, 1 << 20, (1 << 20) + 1, 3 << 20, 9 << 20] {
                let data = noisy(n);
                let raw = compress_at(&data, level).unwrap();
                let (size, window) = frame_header(&raw);
                assert_eq!(size, Some(n as u64), "{n} bytes, level {level}");
                assert_eq!(
                    zstd::zstd_safe::get_frame_content_size(&raw).ok(),
                    Some(Some(n as u64))
                );
                assert!(
                    window <= 1 << ZSTD_WINDOW_LOG,
                    "{n} bytes, level {level}: window {window}"
                );
                assert!(
                    decoder_buffer(size, window) <= (1 << 20) + 393_280,
                    "{n} bytes, level {level}: {}",
                    decoder_buffer(size, window)
                );
                assert_eq!(zstd::decode_all(raw.as_slice()).unwrap(), data);
            }
        }
        let (size, window) = frame_header(&compress(&noisy(9 << 20)).unwrap());
        assert_eq!(
            (size, window, decoder_buffer(size, window)),
            (Some(9 << 20), 1 << 20, 1_441_856)
        );
    }

    fn sample() -> Erp {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        header[4..8].copy_from_slice(&4u32.to_le_bytes());
        let text = b"!pkg sample payload".to_vec();
        let packed = compress(&text).unwrap();
        let mut kind = [0u8; 16];
        kind[..2].copy_from_slice(b"UI");
        let mut res = Resource {
            name: "eaid://ui_package/screens/sample.ui".into(),
            kind,
            unk: vec![0; 6],
            fragments: vec![
                Fragment {
                    name: *b"temp",
                    offset: 0,
                    size: text.len() as u64,
                    flags: 0,
                    compression: ZSTD,
                    packed: packed.len() as u64,
                    raw: packed,
                },
                Fragment {
                    name: *b"syst",
                    offset: 0,
                    size: 3,
                    flags: 0,
                    compression: 0,
                    packed: 3,
                    raw: b"abc".to_vec(),
                },
            ],
            hash: [0; 16],
        };
        res.hash = res.content_hash().unwrap();
        Erp {
            header,
            version: 4,
            resources: vec![res],
        }
    }

    #[test]
    fn write_parse_write_is_identical() {
        let bytes = sample().to_bytes();
        let mut back = Erp::parse(&bytes).unwrap();
        assert_eq!(back.to_bytes(), bytes);
        let res = back.find("eaid://ui_package/screens/sample.ui").unwrap();
        assert_eq!(res.fragments[0].data().unwrap(), b"!pkg sample payload");
        assert_eq!(res.kind(), "UI");
    }

    #[test]
    fn a_cached_compression_matches_a_fresh_one_and_a_damaged_entry_is_redone() {
        let dir = std::env::temp_dir().join(format!("swofty-packed-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let data: Vec<u8> = (0..PACKED_MIN as u32 * 2).map(|i| (i % 251) as u8).collect();
        let fresh = encode(&data, ZSTD_LEVEL).unwrap();
        assert_eq!(compress_cached(&dir, &data, ZSTD_LEVEL).unwrap(), fresh);
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .flat_map(|d| std::fs::read_dir(d.path()).unwrap().flatten())
            .map(|e| e.path())
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(std::fs::read(&entries[0]).unwrap(), fresh);
        assert_eq!(compress_cached(&dir, &data, ZSTD_LEVEL).unwrap(), fresh);
        std::fs::write(&entries[0], &fresh[..fresh.len() / 2]).unwrap();
        assert_eq!(compress_cached(&dir, &data, ZSTD_LEVEL).unwrap(), fresh);
        assert_eq!(std::fs::read(&entries[0]).unwrap(), fresh);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn set_fragment_updates_hash() {
        let mut erp = sample();
        let res = erp.find_mut("eaid://ui_package/screens/sample.ui").unwrap();
        res.set_fragment(0, b"changed").unwrap();
        let hash = res.hash;
        let bytes = erp.to_bytes();
        let back = Erp::parse(&bytes).unwrap();
        let res = &back.resources[0];
        assert_eq!(res.fragments[0].data().unwrap(), b"changed");
        assert_eq!(res.content_hash().unwrap(), hash);
    }

    #[test]
    fn stale_stats_compares_a_bxml_fragment_with_its_tsmd() {
        use crate::bxml::{Document, Element, Kind};
        let doc = |v: &[u8]| Document {
            root: Element {
                kind: Kind::Element,
                pad: 0,
                name: b"track".to_vec(),
                attrs: vec![(b"name".to_vec(), v.to_vec())],
                children: Vec::new(),
                end_pad: 0,
            },
            tail: Vec::new(),
        };
        let stock = doc(b"default");
        let t = &sample().resources[0];
        let spline = |data: &Document| {
            Resource::like(
                "eaid://environment_package/tracks/x/ai/x_ai_track.aispline",
                t,
                &[data.to_bytes(), stock.string_stats(1)],
            )
            .unwrap()
        };
        assert!(spline(&stock).has_stats() && !spline(&stock).stale_stats().unwrap());
        assert!(spline(&doc(b"a longer name")).stale_stats().unwrap());
        assert!(!sample().resources[0].has_stats() && !sample().resources[0].stale_stats().unwrap());

        let mut unchanged = spline(&stock);
        let before = unchanged.fragments[1].data().unwrap();
        assert!(!unchanged.repair_stats().unwrap());
        assert_eq!(unchanged.fragments[1].data().unwrap(), before);

        let edited = doc(b"a longer name");
        let mut res = spline(&edited);
        assert!(res.repair_stats().unwrap());
        assert_eq!(res.fragments[1].data().unwrap(), edited.string_stats(1));
        assert!(!res.stale_stats().unwrap() && !res.repair_stats().unwrap());
        assert_eq!(res.content_hash().unwrap(), res.hash);

        let mut broken = spline(&stock);
        let bytes = stock.to_bytes();
        broken.set_fragment(0, &bytes[..bytes.len() - 3]).unwrap();
        assert!(broken.repair_stats().is_err());
    }

    #[test]
    fn like_takes_the_type_argument_and_fragment_layout_of_a_template() {
        let erp = sample();
        let t = &erp.resources[0];
        let res = Resource::like("eaid://ui_package/x.ui", t, &[b"one".to_vec(), b"two".to_vec()]).unwrap();
        assert_eq!((res.kind, &res.unk), (t.kind, &t.unk));
        for (f, s) in res.fragments.iter().zip(&t.fragments) {
            assert_eq!((f.name, f.flags, f.compression), (s.name, s.flags, s.compression));
        }
        assert_eq!(res.fragments[0].data().unwrap(), b"one");
        assert_ne!(res.fragments[0].raw, b"one");
        assert_eq!(
            (res.fragments[0].size, res.fragments[0].packed),
            (3, res.fragments[0].raw.len() as u64)
        );
        assert_eq!(res.fragments[1].raw, b"two");
        assert_eq!(res.hash, res.content_hash().unwrap());
        assert!(Resource::like("eaid://ui_package/x.ui", t, &[b"one".to_vec()]).is_err());
        assert!(Resource::like("", t, &[b"one".to_vec(), b"two".to_vec()]).is_err());
        assert_eq!(&kind("GfxSurfaceRes").unwrap(), b"GfxSurfaceRes\0\0\0");
        assert!(kind("AIBrakeSettings").is_ok());
        assert!(kind("SixteenBytesLong").is_err());
        assert!(kind("").is_err());
    }

    #[test]
    fn reads_matching_resources_from_a_file() {
        let path = std::env::temp_dir().join(format!("swofty-erp-{}.erp", std::process::id()));
        std::fs::write(&path, sample().to_bytes()).unwrap();
        let found = Erp::read_matching(&path, |n| n.ends_with("sample.ui")).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].fragments[0].data().unwrap(), b"!pkg sample payload");
        assert!(Erp::read_matching(&path, |_| false).unwrap().is_empty());
        std::fs::remove_file(&path).ok();
    }

    fn two() -> Erp {
        let mut erp = sample();
        let mut res = erp.resources[0].clone();
        res.name = "eaid://ui_package/screens/other.ui".into();
        res.set_fragment(0, b"second payload").unwrap();
        erp.resources.push(res);
        erp
    }

    #[test]
    fn streamed_copy_of_every_resource_is_identical() {
        let dir = std::env::temp_dir().join(format!("swofty-erpfile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.erp");
        let bytes = two().to_bytes();
        std::fs::write(&src, &bytes).unwrap();
        let file = ErpFile::open(&src).unwrap();
        let all = file.erp.resources.iter().map(Entry::Copy).collect();
        file.write(all, &dir.join("all.erp")).unwrap();
        assert_eq!(std::fs::read(dir.join("all.erp")).unwrap(), bytes);

        let mut owned = file.load(&file.erp.resources[0]).unwrap();
        owned.name = "eaid://ui_package/screens/renamed.ui".into();
        file.write(
            vec![Entry::Copy(&file.erp.resources[1]), Entry::Owned(owned)],
            &dir.join("subset.erp"),
        )
        .unwrap();
        let back = Erp::parse(&std::fs::read(dir.join("subset.erp")).unwrap()).unwrap();
        let names: Vec<&str> = back.resources.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "eaid://ui_package/screens/other.ui",
                "eaid://ui_package/screens/renamed.ui"
            ]
        );
        assert_eq!(back.resources[0].fragments[0].data().unwrap(), b"second payload");
        assert_eq!(
            back.resources[1].fragments[0].data().unwrap(),
            b"!pkg sample payload"
        );
        assert_eq!(back.resources[1].fragments[1].data().unwrap(), b"abc");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_archive_rewritten_over_itself_replaces_a_read_only_copy() {
        let dir = std::env::temp_dir().join(format!("swofty-erp-in-place-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("databases.erp");
        std::fs::write(&path, two().to_bytes()).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms).unwrap();
        let file = ErpFile::open(&path).unwrap();
        let mut owned = file.load(&file.erp.resources[1]).unwrap();
        owned.set_fragment(0, b"patched").unwrap();
        file.write(
            vec![Entry::Copy(&file.erp.resources[0]), Entry::Owned(owned)],
            &path,
        )
        .unwrap();
        let back = Erp::parse(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(back.resources[1].fragments[0].data().unwrap(), b"patched");
        assert_eq!(
            back.resources[0].fragments[0].data().unwrap(),
            b"!pkg sample payload"
        );
        assert!(!path.with_extension("erp.tmp").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    fn refused(code: i32) -> std::io::Error {
        std::io::Error::from_raw_os_error(code)
    }

    #[test]
    fn retries_a_file_windows_refuses_for_now() {
        let held = |e: &std::io::Error| matches!(e.raw_os_error(), Some(5 | 32));
        let mut waits = Vec::new();
        let mut tries = [refused(5), refused(32)].into_iter();
        let opened = retrying(&held, &mut |d| waits.push(d.as_millis()), || match tries.next() {
            Some(e) => Err(e),
            None => Ok("opened"),
        });
        assert_eq!(opened.unwrap(), "opened");
        assert_eq!(waits, [25, 50]);

        let mut waits = Vec::new();
        let mut calls = 0;
        let err = retrying(
            &held,
            &mut |d| waits.push(d.as_millis()),
            || -> std::io::Result<()> {
                calls += 1;
                Err(refused(5))
            },
        )
        .unwrap_err();
        assert_eq!(err.raw_os_error(), Some(5));
        assert_eq!(calls, BACKOFF_MS.len() + 1);
        assert_eq!(waits, BACKOFF_MS.map(u128::from));

        let mut waits = Vec::new();
        let mut calls = 0;
        let err = retrying(
            &held,
            &mut |d| waits.push(d.as_millis()),
            || -> std::io::Result<()> {
                calls += 1;
                Err(std::io::Error::from(std::io::ErrorKind::NotFound))
            },
        )
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert_eq!((calls, waits.len()), (1, 0));
    }

    #[test]
    fn only_windows_refusals_count_as_held_for_now() {
        assert_eq!(transient(&refused(5)), cfg!(windows));
        assert_eq!(transient(&refused(32)), cfg!(windows));
        assert!(!transient(&refused(2)));
        assert!(!transient(&std::io::Error::from(
            std::io::ErrorKind::PermissionDenied
        )));
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = sample().to_bytes();
        bytes[0] = b'X';
        assert!(matches!(Erp::parse(&bytes), Err(Error::Magic { .. })));
    }
}
