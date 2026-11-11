//! Executable residency and the source used to reload one deployment.

use crate::{
    engine::{Engine, Image},
    meter::{Pool, Running},
    syscall::Table,
};
use anyhow::{Context, Result};
use lru::LruCache;
use parking_lot::Mutex;
use std::{
    fs,
    io::Write,
    ptr,
    sync::{
        Arc, Weak,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tempfile::NamedTempFile;

pub(crate) struct Code {
    cache: Mutex<Residency>,
    capacity: u64,
    idle: Duration,
    next: AtomicU64,
}

struct Residency {
    entries: LruCache<u64, Cached>,
    bytes: u64,
}

struct Cached {
    touched: Instant,
    image: Arc<Image>,
}

impl Residency {
    fn remove(&mut self, id: u64) {
        if let Some(cached) = self.entries.pop(&id) {
            self.bytes -= cached.image.bytes;
        }
    }

    fn pop(&mut self) {
        if let Some((_, cached)) = self.entries.pop_lru() {
            self.bytes -= cached.image.bytes;
        }
    }
}

impl Code {
    pub(crate) fn new(capacity: u64, idle: Duration) -> Arc<Self> {
        let code = Arc::new(Self {
            cache: Mutex::new(Residency {
                entries: LruCache::unbounded(),
                bytes: 0,
            }),
            capacity,
            idle,
            next: AtomicU64::new(0),
        });
        let weak = Arc::downgrade(&code);
        let interval = idle.clamp(Duration::from_millis(1), Duration::from_secs(1));
        thread::Builder::new()
            .name("berm-code-cache".to_owned())
            .spawn(move || {
                loop {
                    thread::sleep(interval);
                    let Some(code) = weak.upgrade() else {
                        return;
                    };
                    code.expire();
                }
            })
            .expect("spawn the code cache maintainer");
        code
    }

    fn get(&self, id: u64) -> Option<Arc<Image>> {
        let mut cache = self.cache.lock();
        if cache
            .entries
            .peek(&id)
            .is_some_and(|entry| entry.touched.elapsed() >= self.idle)
        {
            cache.remove(id);
            return None;
        }
        let cached = cache.entries.get_mut(&id)?;
        cached.touched = Instant::now();
        Some(cached.image.clone())
    }

    fn insert(&self, id: u64, image: Arc<Image>) {
        let bytes = image.bytes;
        let mut cache = self.cache.lock();
        cache.remove(id);
        if bytes > self.capacity || self.idle.is_zero() {
            return;
        }
        while cache.bytes > self.capacity - bytes {
            cache.pop();
        }
        cache.bytes += bytes;
        cache.entries.put(
            id,
            Cached {
                touched: Instant::now(),
                image,
            },
        );
    }

    fn expire(&self) {
        let mut cache = self.cache.lock();
        while cache
            .entries
            .peek_lru()
            .is_some_and(|(_, entry)| entry.touched.elapsed() >= self.idle)
        {
            cache.pop();
        }
    }

    pub(crate) fn reclaim(&self, pool: &Pool) {
        let mut cache = self.cache.lock();
        let ids: Vec<_> = cache
            .entries
            .iter()
            .filter_map(|(id, cached)| {
                cached
                    .image
                    .resident
                    .as_ref()
                    .filter(|resident| ptr::eq(Arc::as_ptr(&resident.pool), pool))
                    .map(|_| *id)
            })
            .collect();
        for id in ids {
            cache.remove(id);
        }
    }

    fn remove(&self, id: u64) {
        self.cache.lock().remove(id);
    }
}

pub(crate) struct Source {
    engine: Engine,
    file: NamedTempFile,
    table: Table,
    id: u64,
    running: Arc<Running>,
    pool: Arc<Pool>,
    latest: Mutex<Weak<Image>>,
}

impl Source {
    pub(crate) fn new(
        engine: &Engine,
        wasm: &[u8],
        table: Table,
        pool: Arc<Pool>,
        running: Arc<Running>,
        compiled: Option<Image>,
    ) -> Result<Arc<Self>> {
        let weak = Arc::downgrade(&engine.code);
        {
            let mut codes = pool.codes.lock();
            codes.retain(|code| code.strong_count() != 0);
            if !codes.iter().any(|code| code.ptr_eq(&weak)) {
                codes.push(weak);
            }
        }
        let mut file = NamedTempFile::new().context("staging a deployed image")?;
        file.write_all(wasm)?;
        let source = Arc::new(Self {
            engine: engine.clone(),
            file,
            table,
            id: engine.code.next.fetch_add(1, Ordering::Relaxed),
            running,
            pool,
            latest: Mutex::default(),
        });
        if let Some(image) = compiled {
            let image = source.track(image);
            *source.latest.lock() = Arc::downgrade(&image);
            source.engine.code.insert(source.id, image);
        }
        Ok(source)
    }

    fn track(&self, mut image: Image) -> Arc<Image> {
        let bytes = image.bytes;
        self.running.loaded.fetch_add(bytes, Ordering::Relaxed);
        self.pool.load(bytes);
        image.resident = Some(Resident {
            bytes,
            running: self.running.clone(),
            pool: self.pool.clone(),
        });
        Arc::new(image)
    }

    pub(crate) async fn executable(self: &Arc<Self>, deadline: Instant) -> Result<Arc<Image>> {
        anyhow::ensure!(
            Instant::now() < deadline,
            "call deadline expired before loading executable"
        );
        if let Some(image) = self
            .engine
            .code
            .get(self.id)
            .or_else(|| self.latest.try_lock().and_then(|latest| latest.upgrade()))
        {
            return Ok(image);
        }
        let source = self.clone();
        tokio::time::timeout_at(
            deadline.into(),
            tokio::task::spawn_blocking(move || {
                let mut latest = source.latest.lock();
                if let Some(image) = latest.upgrade() {
                    return Ok(image);
                }
                anyhow::ensure!(
                    Instant::now() < deadline,
                    "call deadline expired before loading executable"
                );
                let wasm = fs::read(source.file.path()).context("reading a deployed image")?;
                let (image, _) = Image::compile(&source.engine, &wasm, source.table.clone())?;
                let image = source.track(image);
                source.pool.fits(image.bytes)?;
                *latest = Arc::downgrade(&image);
                source.engine.code.insert(source.id, image.clone());
                Ok::<_, anyhow::Error>(image)
            }),
        )
        .await
        .context("call deadline expired while loading executable")?
        .context("executable loader panicked")?
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        self.engine.code.remove(self.id);
    }
}

pub(crate) struct Resident {
    pub bytes: u64,
    running: Arc<Running>,
    pub pool: Arc<Pool>,
}

impl Drop for Resident {
    fn drop(&mut self) {
        self.running.loaded.fetch_sub(self.bytes, Ordering::Relaxed);
        self.pool.unload(self.bytes);
    }
}
