//! Trusted local image programs, supervised without inherited secrets or shell evaluation.
use crate::agent::process::{Exit, Limits, Spec, Stop, Supervisor};
use diary_application::image::*;
use std::{
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct CommandEngine {
    pub program: PathBuf,
    pub root: PathBuf,
    pub timeout: Duration,
}
struct Staging(Option<PathBuf>);
impl Drop for Staging {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = fs::remove_dir_all(path);
        }
    }
}
impl CommandEngine {
    fn execute(
        &self,
        cwd: PathBuf,
        argv: Vec<std::ffi::OsString>,
        env: Vec<(std::ffi::OsString, std::ffi::OsString)>,
        cancel: &diary_application::agent::CancelToken,
        probe: bool,
    ) -> Result<(), EngineError> {
        let mut limits = Limits::probe();
        if !probe {
            limits.wall = self.timeout;
            limits.inactivity = self.timeout;
            limits.record = 1024 * 1024;
            limits.total = 1024 * 1024;
        }
        let o = Supervisor::run(
            Spec {
                executable: self.program.clone(),
                argv,
                env,
                stdin: vec![],
                cwd,
                limits,
                shutdown: None,
            },
            cancel,
            &mut |_| {},
        )
        .map_err(|_| EngineError::Io)?;
        match o.stop {
            Some(Stop::Cancelled) => Err(EngineError::Cancelled),
            Some(Stop::Wall | Stop::Inactivity) => Err(EngineError::Timeout),
            Some(_) => Err(EngineError::Failed),
            None if o.exit == Exit::Success && o.escaped == 0 => Ok(()),
            None => Err(EngineError::Failed),
        }
    }
}
impl ImageEngine for CommandEngine {
    fn check(&self) -> Availability {
        if !self.program.is_absolute()
            || !fs::metadata(&self.program)
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        {
            return Availability::Unavailable;
        }
        if self
            .execute(
                self.root.clone(),
                vec!["--check".into()],
                vec![],
                &Default::default(),
                true,
            )
            .is_ok()
        {
            Availability::Ready
        } else {
            Availability::Unavailable
        }
    }
    fn render(&self, r: &RenderRequest) -> Result<Rendered, EngineError> {
        r.validate()?;
        let dir = self.root.join(format!(
            "image-run-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            crate::lifecycle::random_uuid().map_err(|_| EngineError::Io)?
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|_| EngineError::Io)?;
        let mut staging = Staging(Some(dir.clone()));
        let out = dir.join("image.png");
        self.execute(
            dir.clone(),
            vec![],
            vec![
                ("DIARY_IMAGE_PROMPT".into(), r.prompt.clone().into()),
                ("DIARY_IMAGE_SEED".into(), r.seed.to_string().into()),
                ("DIARY_IMAGE_OUT".into(), out.clone().into()),
            ],
            &r.cancel,
            false,
        )?;
        let (bytes, width, height) = validate(&out)?;
        fs::set_permissions(&out, fs::Permissions::from_mode(0o600))
            .map_err(|_| EngineError::Io)?;
        for item in fs::read_dir(&dir).map_err(|_| EngineError::Io)? {
            let p = item.map_err(|_| EngineError::Io)?.path();
            if p != out {
                if p.is_dir() && !p.is_symlink() {
                    fs::remove_dir_all(p)
                } else {
                    fs::remove_file(p)
                }
                .map_err(|_| EngineError::Io)?;
            }
        }
        staging.0.take();
        Ok(Rendered {
            path: out,
            bytes,
            width,
            height,
            seed: r.seed,
            staging: dir,
        })
    }
}
fn crc(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for b in data {
        c ^= *b as u32;
        for _ in 0..8 {
            c = (c >> 1) ^ (0xedb88320u32.wrapping_mul(c & 1));
        }
    }
    !c
}
pub fn validate(path: &Path) -> Result<(u64, u32, u32), EngineError> {
    let invalid = EngineError::InvalidOutput;
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| invalid)?;
    let m = f.metadata().map_err(|_| invalid)?;
    if !m.is_file() || m.len() > 32 * 1024 * 1024 {
        return Err(invalid);
    }
    let mut b = Vec::new();
    f.by_ref()
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut b)
        .map_err(|_| invalid)?;
    if b.len() > 32 * 1024 * 1024 || b.get(..8) != Some(b"\x89PNG\r\n\x1a\n") {
        return Err(invalid);
    }
    let mut pos = 8;
    let mut dims = None;
    let mut idat = false;
    let mut ended = false;
    let mut palette = false;
    let mut color = 0;
    let mut depth = 0;
    while pos + 12 <= b.len() {
        let n = u32::from_be_bytes(b[pos..pos + 4].try_into().unwrap()) as usize;
        let end = pos
            .checked_add(12 + n)
            .filter(|e| *e <= b.len())
            .ok_or(invalid)?;
        let kind = &b[pos + 4..pos + 8];
        if !kind.iter().all(u8::is_ascii_alphabetic) || !kind[2].is_ascii_uppercase() {
            return Err(invalid);
        }
        if crc(&b[pos + 4..end - 4]) != u32::from_be_bytes(b[end - 4..end].try_into().unwrap()) {
            return Err(invalid);
        }
        if dims.is_none() && kind != b"IHDR" {
            return Err(invalid);
        }
        match kind {
            b"IHDR" => {
                if dims.is_some() || n != 13 {
                    return Err(invalid);
                }
                let w = u32::from_be_bytes(b[pos + 8..pos + 12].try_into().unwrap());
                let h = u32::from_be_bytes(b[pos + 12..pos + 16].try_into().unwrap());
                depth = b[pos + 16];
                color = b[pos + 17];
                let valid_depth = match color {
                    0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
                    2 | 4 | 6 => matches!(depth, 8 | 16),
                    3 => matches!(depth, 1 | 2 | 4 | 8),
                    _ => false,
                };
                if !valid_depth || b[pos + 18] != 0 || b[pos + 19] != 0 || b[pos + 20] > 1 {
                    return Err(invalid);
                }
                if w == 0
                    || h == 0
                    || w > 8192
                    || h > 8192
                    || u64::from(w) * u64::from(h) > 32_000_000
                {
                    return Err(invalid);
                }
                dims = Some((w, h));
            }
            b"PLTE" => {
                if palette
                    || idat
                    || matches!(color, 0 | 4)
                    || n == 0
                    || !n.is_multiple_of(3)
                    || n > 768
                    || (color == 3 && n / 3 > 1usize << depth)
                {
                    return Err(invalid);
                }
                palette = true;
            }
            b"IDAT" => {
                if ended || (color == 3 && !palette) {
                    return Err(invalid);
                }
                idat = true;
            }
            b"IEND" => {
                if n != 0 || !idat || end != b.len() {
                    return Err(invalid);
                }
                pos = end;
                break;
            }
            _ => {
                if kind[0].is_ascii_uppercase() {
                    return Err(invalid);
                }
                if idat {
                    ended = true;
                }
            }
        }
        pos = end;
    }
    if pos != b.len()
        || b.get(pos.saturating_sub(8)..pos.saturating_sub(4)) != Some(b"IEND")
        || !decode(&b)
    {
        return Err(invalid);
    }
    let (w, h) = dims.ok_or(invalid)?;
    Ok((b.len() as u64, w, h))
}
// ImageIO is the system decoder; no third-party decoder dependency.
fn decode(bytes: &[u8]) -> bool {
    use std::ffi::c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFDataCreate(a: *const c_void, b: *const u8, n: isize) -> *const c_void;
        fn CFRelease(p: *const c_void);
        fn CFDataGetLength(d: *const c_void) -> isize;
        static kCFBooleanTrue: *const c_void;
        fn CFDictionaryCreate(
            a: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            n: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> *const c_void;
    }
    #[link(name = "ImageIO", kind = "framework")]
    unsafe extern "C" {
        static kCGImageSourceShouldCacheImmediately: *const c_void;
        fn CGImageSourceGetStatusAtIndex(s: *const c_void, i: usize) -> i32;
        fn CGImageSourceCreateWithData(d: *const c_void, o: *const c_void) -> *const c_void;
        fn CGImageSourceCreateImageAtIndex(
            s: *const c_void,
            i: usize,
            o: *const c_void,
        ) -> *const c_void;
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGImageGetDataProvider(image: *const c_void) -> *const c_void;
        fn CGDataProviderCopyData(provider: *const c_void) -> *const c_void;
        fn CGImageGetBytesPerRow(image: *const c_void) -> usize;
        fn CGImageGetHeight(image: *const c_void) -> usize;
    }
    // SAFETY: immutable bytes live through source creation; each retained object is released once.
    unsafe {
        let d = CFDataCreate(std::ptr::null(), bytes.as_ptr(), bytes.len() as isize);
        if d.is_null() {
            return false;
        }
        let s = CGImageSourceCreateWithData(d, std::ptr::null());
        let keys = [kCGImageSourceShouldCacheImmediately];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            std::ptr::null(),
            std::ptr::null(),
        );
        let image = if s.is_null() || options.is_null() {
            std::ptr::null()
        } else {
            CGImageSourceCreateImageAtIndex(s, 0, options)
        };
        let pixels = if image.is_null() {
            std::ptr::null()
        } else {
            let provider = CGImageGetDataProvider(image);
            if provider.is_null() {
                std::ptr::null()
            } else {
                CGDataProviderCopyData(provider)
            }
        };
        let ok = !pixels.is_null()
            && CFDataGetLength(pixels) > 0
            && CFDataGetLength(pixels) as usize
                == CGImageGetBytesPerRow(image) * CGImageGetHeight(image)
            && CGImageSourceGetStatusAtIndex(s, 0) == 0;
        if !pixels.is_null() {
            CFRelease(pixels);
        }
        if !options.is_null() {
            CFRelease(options);
        }
        if !image.is_null() {
            CFRelease(image)
        }
        if !s.is_null() {
            CFRelease(s)
        }
        CFRelease(d);
        ok
    }
}
pub fn configured(c: &crate::config::Config) -> Vec<(String, String, Arc<dyn ImageEngine>)> {
    c.image_engines
        .iter()
        .map(|(id, e)| {
            (
                id.clone(),
                e.label.clone(),
                Arc::new(CommandEngine {
                    program: e.program.clone(),
                    root: image_root(c),
                    timeout: Duration::from_secs(e.timeout_seconds),
                }) as Arc<dyn ImageEngine>,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    // Deterministic 1x1 PNG with real compressed pixel data.
    const PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2, 0, 0, 5, 0,
        1, 122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    fn chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
        let mut b = (data.len() as u32).to_be_bytes().to_vec();
        b.extend(kind);
        b.extend(data);
        let crc = crc(&b[4..]);
        b.extend(crc.to_be_bytes());
        b
    }
    #[test]
    fn validates_decoding_and_rejects_bad_output() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.png");
        fs::write(&path, PNG).unwrap();
        assert_eq!(validate(&path), Ok((PNG.len() as u64, 1, 1)));
        let link = dir.path().join("link");
        symlink(&path, &link).unwrap();
        assert_eq!(validate(&link), Err(EngineError::InvalidOutput));
        for n in [0, 8, 32, PNG.len() - 1] {
            fs::write(&path, &PNG[..n]).unwrap();
            assert_eq!(validate(&path), Err(EngineError::InvalidOutput));
        }
        let mut bad = PNG.to_vec();
        bad[45] ^= 1;
        fs::write(&path, bad).unwrap();
        assert_eq!(validate(&path), Err(EngineError::InvalidOutput));
    }
    #[test]
    fn png_structure_decode_and_size_limits_are_independent() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("out");
        let header = &PNG[16..29];
        for variant in 0..5 {
            let mut ihdr = header.to_vec();
            if variant == 0 {
                ihdr[..4].copy_from_slice(&8193u32.to_be_bytes());
            }
            let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
            b.extend(chunk(b"IHDR", &ihdr));
            if variant == 1 {
                b.extend(chunk(b"IHDR", &ihdr));
            }
            b.extend(chunk(b"IDAT", b"not compressed pixels"));
            if variant != 2 {
                b.extend(chunk(b"IEND", b""));
            }
            if variant == 3 {
                b.push(0);
            }
            fs::write(&path, b).unwrap();
            assert_eq!(validate(&path), Err(EngineError::InvalidOutput));
        }
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert_eq!(validate(&path), Err(EngineError::InvalidOutput));
        fs::remove_dir(&path).unwrap();
        let f = fs::File::create(&path).unwrap();
        f.set_len(32 * 1024 * 1024 + 1).unwrap();
        assert_eq!(validate(&path), Err(EngineError::InvalidOutput));
    }
    #[test]
    fn rejects_unknown_critical_and_invalid_palette_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.png");
        for (kind, data) in [
            (b"ABCD".as_slice(), b"".as_slice()),
            (b"abca", b""),
            (b"te1t", b""),
            (b"PLTE", b"bad length"),
        ] {
            let mut bytes = PNG[..33].to_vec();
            bytes.extend(chunk(kind, data));
            bytes.extend_from_slice(&PNG[33..]);
            fs::write(&path, bytes).unwrap();
            assert_eq!(validate(&path), Err(EngineError::InvalidOutput), "{kind:?}");
        }
        // Unknown ancillary chunks with a valid reserved bit remain legal.
        let mut bytes = PNG[..33].to_vec();
        bytes.extend(chunk(b"vpAg", b"synthetic"));
        bytes.extend_from_slice(&PNG[33..]);
        fs::write(&path, &bytes).unwrap();
        assert_eq!(validate(&path), Ok((bytes.len() as u64, 1, 1)));
    }
    #[test]
    fn rejects_illegal_headers_and_palette_order_with_valid_compressed_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.png");
        for (offset, value) in [(8, 3), (9, 1), (10, 1), (11, 1), (12, 2)] {
            let mut header = PNG[16..29].to_vec();
            header[offset] = value;
            let mut bytes = PNG[..8].to_vec();
            bytes.extend(chunk(b"IHDR", &header));
            bytes.extend_from_slice(&PNG[33..]);
            fs::write(&path, bytes).unwrap();
            assert_eq!(validate(&path), Err(EngineError::InvalidOutput));
        }
        let palette = chunk(b"PLTE", &[0, 0, 0]);
        // A legal optional palette for RGBA is accepted exactly once, before IDAT.
        let mut bytes = PNG[..33].to_vec();
        bytes.extend_from_slice(&palette);
        bytes.extend_from_slice(&PNG[33..]);
        fs::write(&path, &bytes).unwrap();
        assert_eq!(validate(&path), Ok((bytes.len() as u64, 1, 1)));
        bytes.splice(33..33, palette.clone());
        fs::write(&path, bytes).unwrap();
        assert_eq!(validate(&path), Err(EngineError::InvalidOutput));
        let mut bytes = PNG[..PNG.len() - 12].to_vec();
        bytes.extend(palette);
        bytes.extend_from_slice(&PNG[PNG.len() - 12..]);
        fs::write(&path, bytes).unwrap();
        assert_eq!(validate(&path), Err(EngineError::InvalidOutput));
    }
    #[test]
    fn command_contract_and_private_cleanup() {
        let root = tempfile::tempdir().unwrap();
        let program = root.path().join("engine with spaces");
        let fixture = root.path().join("fixture");
        fs::write(&fixture, PNG).unwrap();
        fs::write(&program,format!("#!/bin/sh\n[ \"$1\" = --check ] && exit 0\n/bin/cp '{}' \"$DIARY_IMAGE_OUT\"\necho \"$DIARY_IMAGE_PROMPT\" >&2\n",fixture.display())).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        let engine = CommandEngine {
            program,
            root: root.path().into(),
            timeout: Duration::from_secs(2),
        };
        assert_eq!(engine.check(), Availability::Ready);
        let result = engine
            .render(&RenderRequest {
                prompt: "private synthetic".into(),
                seed: 42,
                cancel: Default::default(),
            })
            .unwrap();
        let path = result.path.clone();
        assert_eq!(result.seed, 42);
        assert!(path.exists());
        drop(result);
        assert!(!path.exists());
        assert!(fixture.exists());
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use diary_application::agent::Run;
    #[test]
    fn timeout_cancel_flood_and_failed_output_dispose_only_owned_staging() {
        let root = tempfile::tempdir().unwrap();
        let sentinel = root.path().join("sentinel");
        fs::write(&sentinel, b"keep").unwrap();
        let program = root.path().join("engine");
        for (body, expected) in [
            ("exit 3", EngineError::Failed),
            ("exit 0", EngineError::InvalidOutput),
            ("/bin/sleep 10", EngineError::Timeout),
            ("/usr/bin/yes secret", EngineError::Failed),
        ] {
            fs::write(&program, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
            let engine = CommandEngine {
                program: program.clone(),
                root: root.path().into(),
                timeout: Duration::from_millis(300),
            };
            assert_eq!(
                engine
                    .render(&RenderRequest {
                        prompt: "synthetic".into(),
                        seed: 0,
                        cancel: Default::default()
                    })
                    .err(),
                Some(expected)
            );
            assert_eq!(fs::read(&sentinel).unwrap(), b"keep");
            assert!(!fs::read_dir(root.path()).unwrap().any(|p| {
                p.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("image-run-")
            }));
        }
        fs::write(&program, "#!/bin/sh\n/bin/sleep 10\n").unwrap();
        let run = Run::new();
        let cancel = run.token();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            run.request_cancel();
        });
        let engine = CommandEngine {
            program,
            root: root.path().into(),
            timeout: Duration::from_secs(5),
        };
        assert_eq!(
            engine
                .render(&RenderRequest {
                    prompt: "synthetic".into(),
                    seed: 0,
                    cancel
                })
                .err(),
            Some(EngineError::Cancelled)
        );
        thread.join().unwrap();
        assert!(sentinel.exists());
    }
}

fn image_root(c: &crate::config::Config) -> PathBuf {
    c.paths
        .runtime_root
        .join(crate::lifecycle::workspace_key(&c.paths.entries_root))
        .join("image-runs")
}
/// Called only after the exclusive workspace lock is held. Never traverses symlinks.
pub fn prepare(c: &crate::config::Config) -> Result<(), EngineError> {
    use diary_logging::private_fs::PrivateDir;
    let key = crate::lifecycle::workspace_key(&c.paths.entries_root);
    PrivateDir::open(&c.paths.runtime_root)
        .and_then(|d| d.child(&key))
        .and_then(|d| d.child("image-runs"))
        .map_err(|_| EngineError::Io)?;
    for entry in fs::read_dir(image_root(c)).map_err(|_| EngineError::Io)? {
        let entry = entry.map_err(|_| EngineError::Io)?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with("image-run-")
            && entry.file_type().map_err(|_| EngineError::Io)?.is_dir()
        {
            fs::remove_dir_all(entry.path()).map_err(|_| EngineError::Io)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod live {
    use super::*;
    /// Explicit future proof only. Never runs as part of the gate.
    #[test]
    #[ignore = "requires explicit owner-authorized local GPU proof"]
    fn local_render_and_cancel() {
        if std::env::var("DIARY_IMAGE_LIVE").as_deref() != Ok("1") {
            return;
        }
        let program = PathBuf::from(
            std::env::var_os("DIARY_IMAGE_LIVE_PROGRAM").expect("absolute wrapper path"),
        );
        let root = tempfile::tempdir().unwrap();
        let sentinel = root.path().join("sentinel");
        fs::write(&sentinel, b"keep").unwrap();
        let engine = CommandEngine {
            program,
            root: root.path().into(),
            timeout: Duration::from_secs(1800),
        };
        assert_eq!(engine.check(), Availability::Ready);
        let request = RenderRequest {
            prompt: "A blue ceramic cup on a plain wooden table".into(),
            seed: 42,
            cancel: Default::default(),
        };
        let result = engine.render(&request).unwrap();
        assert!(result.bytes > 0);
        drop(result);
        let run = diary_application::agent::Run::new();
        let cancel = run.token();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(5));
            run.request_cancel();
        });
        assert_eq!(
            engine.render(&RenderRequest { cancel, ..request }).err(),
            Some(EngineError::Cancelled)
        );
        thread.join().unwrap();
        assert!(sentinel.exists());
    }
}
