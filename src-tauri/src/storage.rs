use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::models::{Clip, ClipKind, ClipStats, Pin, UpsertOutcome};
use crate::util;

/// v1 表结构。迁移靠 `PRAGMA user_version` 推进，后续加列时追加 SCHEMA_V2。
const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS clips (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  kind          TEXT NOT NULL,
  hash          TEXT NOT NULL UNIQUE,
  text          TEXT,
  preview       TEXT NOT NULL,
  image_rel     TEXT,
  thumb_rel     TEXT,
  width         INTEGER,
  height        INTEGER,
  bytes         INTEGER NOT NULL DEFAULT 0,
  source        TEXT NOT NULL DEFAULT 'clipboard',
  created_at    TEXT NOT NULL,
  last_used_at  TEXT NOT NULL,
  use_count     INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_clips_last_used ON clips(last_used_at DESC);
CREATE INDEX IF NOT EXISTS idx_clips_kind ON clips(kind);

CREATE TABLE IF NOT EXISTS pins (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  clip_id    INTEGER NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
  x          INTEGER NOT NULL,
  y          INTEGER NOT NULL,
  w          INTEGER NOT NULL,
  h          INTEGER NOT NULL,
  zoom       REAL NOT NULL DEFAULT 1.0,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pins_clip ON pins(clip_id);
"#;

const CLIP_COLUMNS: &str = "id, kind, text, preview, image_rel, thumb_rel, width, height, bytes, source, created_at, last_used_at, use_count";

/// 一次淘汰的结果
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvictReport {
    pub removed: usize,
    pub freed_bytes: i64,
}

/// SQLite 存储。`Mutex<Connection>` 满足 Tauri `State: Send + Sync`。
pub struct Storage {
    conn: Mutex<Connection>,
    data_dir: PathBuf,
}

impl Storage {
    pub fn new(data_dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(data_dir)
            .map_err(|e| format!("创建数据目录 {} 失败: {e}", data_dir.display()))?;
        fs::create_dir_all(data_dir.join("images")).map_err(|e| e.to_string())?;
        fs::create_dir_all(data_dir.join("thumbs")).map_err(|e| e.to_string())?;

        let conn = Connection::open(data_dir.join("ppclip.db"))
            .map_err(|e| format!("打开数据库失败: {e}"))?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
            .map_err(db_err)?;
        migrate(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
            data_dir: data_dir.to_path_buf(),
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ---------------------------------------------------------------- clips

    /// 文本入库。命中相同内容时只置顶 + 计数 +1，不产生重复条目。
    pub fn upsert_text(&self, text: &str, source: &str) -> Result<UpsertOutcome, String> {
        let hash = util::sha256_hex(text.as_bytes());
        let preview = util::truncate_preview(text, 120);
        let now = util::now_rfc3339();

        let mut conn = self.lock();
        let tx = conn.transaction().map_err(db_err)?;

        let existing = find_by_hash(&tx, &hash)?;
        let outcome = match existing {
            Some(id) => {
                bump(&tx, id, &now)?;
                UpsertOutcome { id, created: false }
            }
            None => {
                tx.execute(
                    "INSERT INTO clips (kind, hash, text, preview, source, created_at, last_used_at, use_count)
                     VALUES ('text', ?1, ?2, ?3, ?4, ?5, ?5, 1)",
                    params![hash, text, preview, source, now],
                )
                .map_err(db_err)?;
                UpsertOutcome {
                    id: tx.last_insert_rowid(),
                    created: true,
                }
            }
        };

        tx.commit().map_err(db_err)?;
        Ok(outcome)
    }

    /// 图片入库。文件先落盘再写行，保证「有行必有文件」；
    /// 反过来则会出现指向空文件的死条目。
    pub fn upsert_image(
        &self,
        png: &[u8],
        thumb: &[u8],
        width: u32,
        height: u32,
        source: &str,
    ) -> Result<UpsertOutcome, String> {
        let hash = util::sha256_hex(png);
        let now = util::now_rfc3339();
        let image_rel = format!("images/{hash}.png");
        let thumb_rel = format!("thumbs/{hash}.png");
        let preview = format!("[图片 {width}×{height}]");

        let mut conn = self.lock();
        let tx = conn.transaction().map_err(db_err)?;

        let existing = find_by_hash(&tx, &hash)?;
        let outcome = match existing {
            Some(id) => {
                bump(&tx, id, &now)?;
                UpsertOutcome { id, created: false }
            }
            None => {
                write_file(&self.data_dir.join(&image_rel), png)?;
                write_file(&self.data_dir.join(&thumb_rel), thumb)?;

                tx.execute(
                    "INSERT INTO clips (kind, hash, preview, image_rel, thumb_rel, width, height, bytes, source, created_at, last_used_at, use_count)
                     VALUES ('image', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, 1)",
                    params![
                        hash,
                        preview,
                        image_rel,
                        thumb_rel,
                        width as i64,
                        height as i64,
                        png.len() as i64,
                        source,
                        now
                    ],
                )
                .map_err(db_err)?;

                UpsertOutcome {
                    id: tx.last_insert_rowid(),
                    created: true,
                }
            }
        };

        tx.commit().map_err(db_err)?;
        Ok(outcome)
    }

    /// 文件条目入库（kind = file）。
    ///
    /// 路径列表序列化成 JSON 存进 `text` 列：这样搜索（`text LIKE ?`）能直接命中文件名，
    /// 也不用为「一组文件」单开表 / 加列。`bytes` 记所有文件大小之和，
    /// 只用于界面显示「总共多大」，**不参与图片容量淘汰**（那条只统计 kind = 'image'）。
    pub fn upsert_files(&self, paths: &[String], source: &str) -> Result<UpsertOutcome, String> {
        let paths = util::normalize_files(paths);
        if paths.is_empty() {
            return Err("没有可记录的文件".to_string());
        }

        let payload = util::files_payload(&paths);
        let hash = util::files_key(&paths);
        let preview = util::files_preview(&paths);
        let bytes: i64 = paths
            .iter()
            .filter_map(|path| fs::metadata(path).ok())
            .map(|meta| meta.len() as i64)
            .sum();
        let now = util::now_rfc3339();

        let mut conn = self.lock();
        let tx = conn.transaction().map_err(db_err)?;

        let existing = find_by_hash(&tx, &hash)?;
        let outcome = match existing {
            Some(id) => {
                bump(&tx, id, &now)?;
                UpsertOutcome { id, created: false }
            }
            None => {
                tx.execute(
                    "INSERT INTO clips (kind, hash, text, preview, bytes, source, created_at, last_used_at, use_count)
                     VALUES ('file', ?1, ?2, ?3, ?4, ?5, ?6, ?6, 1)",
                    params![hash, payload, preview, bytes, source, now],
                )
                .map_err(db_err)?;
                UpsertOutcome {
                    id: tx.last_insert_rowid(),
                    created: true,
                }
            }
        };

        tx.commit().map_err(db_err)?;
        Ok(outcome)
    }

    /// 置顶一条已有条目（点击复制时调用）
    pub fn touch(&self, id: i64) -> Result<(), String> {
        let conn = self.lock();
        let now = util::now_rfc3339();
        conn.execute(
            "UPDATE clips SET last_used_at = ?1, use_count = use_count + 1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn get(&self, id: i64) -> Result<Clip, String> {
        let conn = self.lock();
        conn.query_row(
            &format!("SELECT {CLIP_COLUMNS} FROM clips WHERE id = ?1"),
            params![id],
            row_to_clip,
        )
        .optional()
        .map_err(db_err)?
        .ok_or_else(|| format!("条目 {id} 不存在"))
    }

    /// 列表 / 搜索。`query` 与 `kind` 都可选。
    pub fn list(
        &self,
        query: Option<&str>,
        kind: Option<ClipKind>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Clip>, String> {
        let pattern = query
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(like_pattern);
        let kind_filter = kind.map(ClipKind::as_str);

        let conn = self.lock();
        let sql = format!(
            r#"SELECT {CLIP_COLUMNS} FROM clips
               WHERE (?1 IS NULL OR text LIKE ?1 ESCAPE '\' OR preview LIKE ?1 ESCAPE '\')
                 AND (?2 IS NULL OR kind = ?2)
               ORDER BY last_used_at DESC, id DESC
               LIMIT ?3 OFFSET ?4"#
        );
        let mut stmt = conn.prepare(&sql).map_err(db_err)?;
        let rows = stmt
            .query_map(
                params![pattern, kind_filter, limit.max(0), offset.max(0)],
                row_to_clip,
            )
            .map_err(db_err)?;

        let mut clips = Vec::new();
        for row in rows {
            clips.push(row.map_err(db_err)?);
        }
        Ok(clips)
    }

    /// 删除单条，同时清掉它的图片文件。返回被级联删除的贴图 id，便于关窗口。
    pub fn delete(&self, id: i64) -> Result<Vec<i64>, String> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(db_err)?;

        let paths = clip_paths(&tx, id)?;
        let pin_ids = pin_ids_for_clip(&tx, id)?;
        tx.execute("DELETE FROM clips WHERE id = ?1", params![id])
            .map_err(db_err)?;
        tx.commit().map_err(db_err)?;

        if let Some((image, thumb)) = paths {
            remove_file(&self.data_dir, image.as_deref());
            remove_file(&self.data_dir, thumb.as_deref());
        }
        Ok(pin_ids)
    }

    /// 清空（可按类型）。返回清理的条目数。
    pub fn clear(&self, kind: Option<ClipKind>) -> Result<usize, String> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(db_err)?;

        let kind_filter = kind.map(ClipKind::as_str);
        let mut victims: Vec<(Option<String>, Option<String>)> = Vec::new();
        {
            let mut stmt = tx
                .prepare("SELECT image_rel, thumb_rel FROM clips WHERE (?1 IS NULL OR kind = ?1)")
                .map_err(db_err)?;
            let rows = stmt
                .query_map(params![kind_filter], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(db_err)?;
            for row in rows {
                victims.push(row.map_err(db_err)?);
            }
        }

        let removed = tx
            .execute(
                "DELETE FROM clips WHERE (?1 IS NULL OR kind = ?1)",
                params![kind_filter],
            )
            .map_err(db_err)?;
        tx.commit().map_err(db_err)?;

        for (image, thumb) in victims {
            remove_file(&self.data_dir, image.as_deref());
            remove_file(&self.data_dir, thumb.as_deref());
        }
        Ok(removed)
    }

    pub fn stats(&self) -> Result<ClipStats, String> {
        let conn = self.lock();
        conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN kind = 'text' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN kind = 'image' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN kind = 'file' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN kind = 'image' THEN bytes ELSE 0 END), 0)
             FROM clips",
            [],
            |r| {
                Ok(ClipStats {
                    total: r.get(0)?,
                    text_count: r.get(1)?,
                    image_count: r.get(2)?,
                    file_count: r.get(3)?,
                    image_bytes: r.get(4)?,
                })
            },
        )
        .map_err(db_err)
    }

    /// 按条数上限 + 图片容量上限淘汰。有贴图引用的条目一律不动。
    pub fn evict(&self, max_items: i64, max_image_mb: i64) -> Result<EvictReport, String> {
        self.evict_with_limits(max_items, max_image_mb.max(0) * 1024 * 1024)
    }

    /// `evict` 的实现体：容量以字节传入，方便单测用很小的阈值验证。
    fn evict_with_limits(&self, max_items: i64, max_image_bytes: i64) -> Result<EvictReport, String> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(db_err)?;

        let pinned = pinned_clip_ids(&tx)?;
        let mut all: Vec<ClipRow> = Vec::new();
        {
            let mut stmt = tx
                .prepare(
                    "SELECT id, kind, image_rel, thumb_rel, bytes FROM clips
                     ORDER BY last_used_at ASC, id ASC",
                )
                .map_err(db_err)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(ClipRow {
                        id: r.get(0)?,
                        kind: r.get(1)?,
                        image_rel: r.get(2)?,
                        thumb_rel: r.get(3)?,
                        bytes: r.get(4)?,
                    })
                })
                .map_err(db_err)?;
            for row in rows {
                all.push(row.map_err(db_err)?);
            }
        }

        // 候选按「最久未使用」升序，已贴图的跳过
        let candidates: Vec<&ClipRow> = all.iter().filter(|c| !pinned.contains(&c.id)).collect();

        let mut victims: HashSet<i64> = HashSet::new();

        // 1) 条数超限
        let overflow = all.len() as i64 - max_items.max(1);
        if overflow > 0 {
            for row in candidates.iter().take(overflow as usize) {
                victims.insert(row.id);
            }
        }

        // 2) 图片容量超限
        let mut image_bytes: i64 = all
            .iter()
            .filter(|c| c.kind == "image" && !victims.contains(&c.id))
            .map(|c| c.bytes)
            .sum();
        if image_bytes > max_image_bytes {
            for row in candidates.iter().filter(|c| c.kind == "image") {
                if image_bytes <= max_image_bytes {
                    break;
                }
                if victims.insert(row.id) {
                    image_bytes -= row.bytes;
                }
            }
        }

        let mut report = EvictReport::default();
        let mut files: Vec<(Option<String>, Option<String>)> = Vec::new();
        for row in all.iter().filter(|c| victims.contains(&c.id)) {
            tx.execute("DELETE FROM clips WHERE id = ?1", params![row.id])
                .map_err(db_err)?;
            report.removed += 1;
            report.freed_bytes += row.bytes;
            files.push((row.image_rel.clone(), row.thumb_rel.clone()));
        }
        tx.commit().map_err(db_err)?;

        for (image, thumb) in files {
            remove_file(&self.data_dir, image.as_deref());
            remove_file(&self.data_dir, thumb.as_deref());
        }
        Ok(report)
    }

    // ----------------------------------------------------------------- pins

    pub fn create_pin(
        &self,
        clip_id: i64,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        zoom: f64,
    ) -> Result<i64, String> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO pins (clip_id, x, y, w, h, zoom, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![clip_id, x, y, w, h, zoom, util::now_rfc3339()],
        )
        .map_err(db_err)?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_pins(&self) -> Result<Vec<Pin>, String> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, clip_id, x, y, w, h, zoom, created_at FROM pins ORDER BY id ASC",
            )
            .map_err(db_err)?;
        let rows = stmt.query_map([], row_to_pin).map_err(db_err)?;

        let mut pins = Vec::new();
        for row in rows {
            pins.push(row.map_err(db_err)?);
        }
        Ok(pins)
    }

    pub fn get_pin(&self, id: i64) -> Result<Pin, String> {
        let conn = self.lock();
        conn.query_row(
            "SELECT id, clip_id, x, y, w, h, zoom, created_at FROM pins WHERE id = ?1",
            params![id],
            row_to_pin,
        )
        .optional()
        .map_err(db_err)?
        .ok_or_else(|| format!("贴图 {id} 不存在"))
    }

    pub fn update_pin_geometry(
        &self,
        id: i64,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        zoom: f64,
    ) -> Result<(), String> {
        let conn = self.lock();
        conn.execute(
            "UPDATE pins SET x = ?1, y = ?2, w = ?3, h = ?4, zoom = ?5 WHERE id = ?6",
            params![x, y, w, h, zoom, id],
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn delete_pin(&self, id: i64) -> Result<(), String> {
        let conn = self.lock();
        conn.execute("DELETE FROM pins WHERE id = ?1", params![id])
            .map_err(db_err)?;
        Ok(())
    }
}

/// 淘汰时的行快照
struct ClipRow {
    id: i64,
    kind: String,
    image_rel: Option<String>,
    thumb_rel: Option<String>,
    bytes: i64,
}

fn migrate(conn: &Connection) -> Result<(), String> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(db_err)?;

    if version < 1 {
        conn.execute_batch(SCHEMA_V1).map_err(db_err)?;
        conn.execute_batch("PRAGMA user_version = 1").map_err(db_err)?;
        log::info!("数据库已初始化到 v1");
    }
    Ok(())
}

fn db_err(err: rusqlite::Error) -> String {
    format!("数据库错误: {err}")
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|e| format!("写入 {} 失败: {e}", path.display()))
}

fn remove_file(data_dir: &Path, rel: Option<&str>) {
    let Some(rel) = rel else { return };
    let path = data_dir.join(rel);
    if let Err(err) = fs::remove_file(&path) {
        if err.kind() != std::io::ErrorKind::NotFound {
            log::warn!("删除文件 {} 失败: {err}", path.display());
        }
    }
}

/// LIKE 通配符转义，避免用户搜 `%` 时把整库都匹配出来
fn like_pattern(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn find_by_hash(conn: &Connection, hash: &str) -> Result<Option<i64>, String> {
    conn.query_row("SELECT id FROM clips WHERE hash = ?1", params![hash], |r| {
        r.get(0)
    })
    .optional()
    .map_err(db_err)
}

fn bump(conn: &Connection, id: i64, now: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE clips SET last_used_at = ?1, use_count = use_count + 1 WHERE id = ?2",
        params![now, id],
    )
    .map_err(db_err)?;
    Ok(())
}

fn clip_paths(
    conn: &Connection,
    id: i64,
) -> Result<Option<(Option<String>, Option<String>)>, String> {
    conn.query_row(
        "SELECT image_rel, thumb_rel FROM clips WHERE id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(db_err)
}

fn pin_ids_for_clip(conn: &Connection, clip_id: i64) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare("SELECT id FROM pins WHERE clip_id = ?1")
        .map_err(db_err)?;
    let rows = stmt.query_map(params![clip_id], |r| r.get(0)).map_err(db_err)?;

    let mut ids = Vec::new();
    for row in rows {
        ids.push(row.map_err(db_err)?);
    }
    Ok(ids)
}

fn pinned_clip_ids(conn: &Connection) -> Result<HashSet<i64>, String> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT clip_id FROM pins")
        .map_err(db_err)?;
    let rows = stmt.query_map([], |r| r.get(0)).map_err(db_err)?;

    let mut ids = HashSet::new();
    for row in rows {
        ids.insert(row.map_err(db_err)?);
    }
    Ok(ids)
}

fn row_to_clip(row: &Row<'_>) -> rusqlite::Result<Clip> {
    let kind_raw: String = row.get("kind")?;
    let kind = ClipKind::parse(&kind_raw).unwrap_or(ClipKind::Text);
    let text: Option<String> = row.get("text")?;

    // 文件条目的路径列表就是 `text` 里的 JSON。解析失败（旧数据 / 手工改库）时给空数组，
    // 宁可显示「0 个文件」也不要整个列表读不出来。
    let files = if kind == ClipKind::File {
        Some(
            text.as_deref()
                .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
                .unwrap_or_default(),
        )
    } else {
        None
    };

    Ok(Clip {
        id: row.get("id")?,
        kind,
        text,
        files,
        preview: row.get("preview")?,
        image_rel: row.get("image_rel")?,
        thumb_rel: row.get("thumb_rel")?,
        width: row.get("width")?,
        height: row.get("height")?,
        bytes: row.get("bytes")?,
        source: row.get("source")?,
        created_at: row.get("created_at")?,
        last_used_at: row.get("last_used_at")?,
        use_count: row.get("use_count")?,
    })
}

fn row_to_pin(row: &Row<'_>) -> rusqlite::Result<Pin> {
    Ok(Pin {
        id: row.get(0)?,
        clip_id: row.get(1)?,
        x: row.get(2)?,
        y: row.get(3)?,
        w: row.get(4)?,
        h: row.get(5)?,
        zoom: row.get(6)?,
        created_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// 每个测试一个独立目录，互不干扰。
    ///
    /// 放在 crate 的 `target/test-tmp` 下而不是系统临时目录：后者在受限环境里
    /// 可能没有写权限，而且这里的产物本来就在 .gitignore 覆盖范围内。
    fn temp_storage() -> (Storage, PathBuf) {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp")
            .join(format!("ppclip-test-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let storage = Storage::new(&dir).expect("建立测试库失败");
        (storage, dir)
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    impl Storage {
        /// 测试专用：直接改写时间戳，让淘汰顺序确定
        fn force_last_used(&self, id: i64, ts: &str) {
            let conn = self.lock();
            conn.execute(
                "UPDATE clips SET last_used_at = ?1 WHERE id = ?2",
                params![ts, id],
            )
            .unwrap();
        }

        fn count(&self) -> i64 {
            let conn = self.lock();
            conn.query_row("SELECT COUNT(*) FROM clips", [], |r| r.get(0))
                .unwrap()
        }
    }

    fn ts(sec: u32) -> String {
        format!("2026-01-01T00:00:{sec:02}+00:00")
    }

    fn fake_png(len: usize) -> Vec<u8> {
        // 内容不影响逻辑：hash 用的是原始字节
        let mut v = vec![0x89, b'P', b'N', b'G'];
        v.resize(len, 0x7f);
        v
    }

    #[test]
    fn text_dedupe_bumps_use_count_instead_of_duplicating() {
        let (storage, dir) = temp_storage();

        let first = storage.upsert_text("hello", "clipboard").unwrap();
        assert!(first.created);

        storage.force_last_used(first.id, &ts(1));
        let second = storage.upsert_text("hello", "clipboard").unwrap();
        assert!(!second.created);
        assert_eq!(first.id, second.id);

        assert_eq!(storage.count(), 1);
        let clip = storage.get(first.id).unwrap();
        assert_eq!(clip.use_count, 2);
        assert!(clip.last_used_at > ts(1), "命中后应置顶");
        assert_eq!(clip.kind, ClipKind::Text);
        assert_eq!(clip.text.as_deref(), Some("hello"));

        cleanup(&dir);
    }

    #[test]
    fn eviction_drops_oldest_and_keeps_pinned() {
        let (storage, dir) = temp_storage();

        let mut ids = Vec::new();
        for i in 0..10u32 {
            let outcome = storage.upsert_text(&format!("clip-{i}"), "clipboard").unwrap();
            storage.force_last_used(outcome.id, &ts(i));
            ids.push(outcome.id);
        }

        // 钉住最旧的一条：它必须活下来
        storage.create_pin(ids[0], 10, 10, 200, 120, 1.0).unwrap();

        let report = storage.evict(5, 200).unwrap();
        assert_eq!(report.removed, 5, "10 条压到 5 条，被钉住的算在内");

        let remaining: Vec<i64> = storage
            .list(None, None, 100, 0)
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(remaining.len(), 5);
        assert!(remaining.contains(&ids[0]), "被贴图引用的条目不能被淘汰");
        assert!(remaining.contains(&ids[9]));
        assert!(!remaining.contains(&ids[1]), "最旧的非钉住条目应被淘汰");

        cleanup(&dir);
    }

    #[test]
    fn eviction_frees_image_bytes_and_removes_files() {
        let (storage, dir) = temp_storage();

        let mut ids = Vec::new();
        for i in 0..3u32 {
            let png = fake_png(2000 + i as usize);
            let outcome = storage
                .upsert_image(&png, b"thumb", 300, 200, "clipboard")
                .unwrap();
            storage.force_last_used(outcome.id, &ts(i));
            ids.push(outcome.id);
        }

        let image_rel = storage.get(ids[0]).unwrap().image_rel.unwrap();
        assert!(dir.join(&image_rel).exists());

        // 总量约 6000 字节，限制 2500 → 必须淘汰最旧的
        let report = storage.evict_with_limits(100, 2500).unwrap();
        assert!(report.removed >= 1);
        assert!(report.freed_bytes > 0);
        assert!(!dir.join(&image_rel).exists(), "淘汰后图片文件应删除");

        let stats = storage.stats().unwrap();
        assert!(stats.image_bytes <= 2500, "剩余图片字节应降到上限内");

        cleanup(&dir);
    }

    #[test]
    fn image_dedupe_does_not_rewrite_files() {
        let (storage, dir) = temp_storage();
        let png = fake_png(1000);

        let first = storage.upsert_image(&png, b"t", 10, 10, "screenshot").unwrap();
        let second = storage.upsert_image(&png, b"t", 10, 10, "screenshot").unwrap();

        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.id, second.id);
        assert_eq!(storage.count(), 1);

        let clip = storage.get(first.id).unwrap();
        assert_eq!(clip.width, Some(10));
        assert_eq!(clip.source, "screenshot");
        assert_eq!(clip.preview, "[图片 10×10]");

        cleanup(&dir);
    }

    #[test]
    fn file_entries_round_trip_paths_instead_of_json() {
        let (storage, dir) = temp_storage();

        // 真实文件：bytes 要能算出来
        let real = dir.join("报告.docx");
        fs::write(&real, b"hello").unwrap();
        let real_str = real.to_string_lossy().to_string();

        let paths = vec![real_str.clone(), "D:\\不存在.png".to_string()];
        let first = storage.upsert_files(&paths, "clipboard").unwrap();
        assert!(first.created);
        assert_eq!(storage.count(), 1);

        let clip = storage.get(first.id).unwrap();
        assert_eq!(clip.kind, ClipKind::File);
        assert_eq!(clip.files.as_deref(), Some(paths.as_slice()));
        assert_eq!(clip.bytes, 5, "只统计真实存在的那一个文件");
        assert_eq!(clip.preview, "报告.docx 等 2 个文件");
        // 路径原样留在 text 里 → 搜索文件名能命中
        assert!(clip.text.as_deref().unwrap().contains("报告.docx"));

        // 同一组文件（大小写不同、顺序相同）再入一次 → 只置顶不重复
        let again = storage
            .upsert_files(&[real_str.to_uppercase(), "D:\\不存在.png".to_string()], "clipboard")
            .unwrap();
        assert!(!again.created);
        assert_eq!(again.id, first.id);

        let stats = storage.stats().unwrap();
        assert_eq!(stats.file_count, 1);
        assert_eq!(stats.text_count, 0);
        assert_eq!(storage.list(None, Some(ClipKind::File), 50, 0).unwrap().len(), 1);

        cleanup(&dir);
    }

    #[test]
    fn empty_file_list_is_rejected() {
        let (storage, dir) = temp_storage();
        assert!(storage.upsert_files(&[], "clipboard").is_err());
        assert!(storage.upsert_files(&["   ".to_string()], "clipboard").is_err());
        assert_eq!(storage.count(), 0);
        cleanup(&dir);
    }

    #[test]
    fn delete_removes_row_files_and_cascades_pins() {
        let (storage, dir) = temp_storage();

        let png = fake_png(500);
        let outcome = storage.upsert_image(&png, b"t", 4, 4, "clipboard").unwrap();
        let rel = storage.get(outcome.id).unwrap().image_rel.unwrap();
        let pin_id = storage
            .create_pin(outcome.id, 1, 2, 100, 80, 1.0)
            .unwrap();

        let cascaded = storage.delete(outcome.id).unwrap();
        assert_eq!(cascaded, vec![pin_id]);
        assert_eq!(storage.count(), 0);
        assert!(!dir.join(&rel).exists());
        assert!(storage.list_pins().unwrap().is_empty());
        assert!(storage.delete_pin(pin_id).is_ok(), "重复删除不应报错");

        cleanup(&dir);
    }

    #[test]
    fn search_filters_by_text_and_kind() {
        let (storage, dir) = temp_storage();

        storage.upsert_text("你好，世界", "clipboard").unwrap();
        storage.upsert_text("hello world", "clipboard").unwrap();
        storage.upsert_text("100% 完成", "clipboard").unwrap();
        storage
            .upsert_image(&fake_png(300), b"t", 8, 8, "clipboard")
            .unwrap();

        assert_eq!(storage.list(Some("世界"), None, 50, 0).unwrap().len(), 1);
        assert_eq!(storage.list(Some("world"), None, 50, 0).unwrap().len(), 1);
        assert_eq!(storage.list(None, Some(ClipKind::Image), 50, 0).unwrap().len(), 1);
        assert_eq!(storage.list(None, Some(ClipKind::Text), 50, 0).unwrap().len(), 3);

        // `%` 是 LIKE 通配符，必须按字面量搜索
        assert_eq!(storage.list(Some("100%"), None, 50, 0).unwrap().len(), 1);
        assert_eq!(storage.list(Some("%"), None, 50, 0).unwrap().len(), 1);

        cleanup(&dir);
    }

    #[test]
    fn clear_by_kind_only_removes_that_kind() {
        let (storage, dir) = temp_storage();

        storage.upsert_text("a", "clipboard").unwrap();
        storage.upsert_text("b", "clipboard").unwrap();
        let image = storage
            .upsert_image(&fake_png(400), b"t", 5, 5, "clipboard")
            .unwrap();
        let rel = storage.get(image.id).unwrap().image_rel.unwrap();

        assert_eq!(storage.clear(Some(ClipKind::Text)).unwrap(), 2);
        assert_eq!(storage.count(), 1);
        assert!(dir.join(&rel).exists(), "清文本不应动图片文件");

        assert_eq!(storage.clear(None).unwrap(), 1);
        assert_eq!(storage.count(), 0);
        assert!(!dir.join(&rel).exists());

        cleanup(&dir);
    }

    #[test]
    fn clear_removes_pins_and_image_files() {
        let (storage, dir) = temp_storage();

        let clip = storage.upsert_text("pinned", "clipboard").unwrap();
        let pin_id = storage.create_pin(clip.id, 10, 20, 320, 240, 1.0).unwrap();
        let image = storage
            .upsert_image(&fake_png(500), b"t", 6, 4, "clipboard")
            .unwrap();
        let image_clip = storage.get(image.id).unwrap();
        let image_rel = image_clip.image_rel.unwrap();
        let thumb_rel = image_clip.thumb_rel.unwrap();

        assert_eq!(storage.clear(None).unwrap(), 2);
        assert_eq!(storage.count(), 0);
        assert!(storage.list_pins().unwrap().is_empty());
        assert!(storage.get_pin(pin_id).is_err());
        assert!(!dir.join(&image_rel).exists());
        assert!(!dir.join(&thumb_rel).exists());

        cleanup(&dir);
    }

    #[test]
    fn stats_aggregate_counts_and_bytes() {
        let (storage, dir) = temp_storage();

        storage.upsert_text("a", "clipboard").unwrap();
        storage.upsert_image(&fake_png(1000), b"t", 1, 1, "clipboard").unwrap();
        storage.upsert_image(&fake_png(2000), b"t", 1, 1, "clipboard").unwrap();

        let stats = storage.stats().unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.text_count, 1);
        assert_eq!(stats.image_count, 2);
        assert_eq!(stats.image_bytes, 3000);

        cleanup(&dir);
    }

    #[test]
    fn pin_geometry_round_trip() {
        let (storage, dir) = temp_storage();
        let clip = storage.upsert_text("pin me", "clipboard").unwrap();

        let pin_id = storage.create_pin(clip.id, 100, 200, 320, 240, 1.5).unwrap();
        storage
            .update_pin_geometry(pin_id, 150, 250, 400, 300, 2.0)
            .unwrap();

        let pin = storage.get_pin(pin_id).unwrap();
        assert_eq!((pin.x, pin.y, pin.w, pin.h), (150, 250, 400, 300));
        assert_eq!(pin.zoom, 2.0);
        assert_eq!(pin.clip_id, clip.id);

        cleanup(&dir);
    }
}
