use anyhow::Result;
use log::{debug, info};
use models::local_state::{AppUiState, WindowState};
use rusqlite::{Connection, params};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub struct LocalStateManager {
    db_path: PathBuf,
}

impl LocalStateManager {
    #[must_use]
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            db_path: config_dir.join("state.db"),
        }
    }

    /// 初始化数据库表 (如果不存在)
    pub fn init(&self) -> Result<()> {
        info!("本地状态管理: 初始化状态数据库 (路径: {:?})", self.db_path);
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS ui_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS pdf_state (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                page_index INTEGER NOT NULL,
                zoom_level REAL NOT NULL,
                offset_y REAL NOT NULL DEFAULT 0.0,
                fit_to_width INTEGER NOT NULL DEFAULT 0,
                is_left_sidebar_open INTEGER NOT NULL DEFAULT 0,
                is_right_sidebar_open INTEGER NOT NULL DEFAULT 0,
                left_sidebar_width REAL NOT NULL DEFAULT 240.0,
                right_sidebar_width REAL NOT NULL DEFAULT 300.0,
                last_read_at INTEGER NOT NULL,
                auto_translate INTEGER NOT NULL DEFAULT 1
            )",
            [],
        )?;

        // 执行数据库迁移（替代旧的 ad-hoc 列检测循环）
        crate::migration::run_migrations(
            &conn,
            &self.db_path,
            &crate::migration::all_migrations(),
        )?;
        debug!("本地状态管理: 状态数据库初始化完成");
        Ok(())
    }

    /// 从数据库加载所有状态到内存
    pub fn load_all(&self) -> Result<AppUiState> {
        debug!("本地状态管理: 正在从数据库加载所有 UI 状态");
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare("SELECT key, value FROM ui_state")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut state = AppUiState::default();
        let mut window_state = WindowState::default();

        for row in rows {
            let (key, value) = row?;
            match key.as_str() {
                "expanded_folder_ids" => {
                    if let Ok(ids) = serde_json::from_str::<HashSet<String>>(&value) {
                        state.expanded_folder_ids = ids;
                    }
                }
                "selected_sidebar_item" => {
                    state.selected_sidebar_item = Some(value);
                }
                "sort_field" => {
                    state.sort_field = Some(value);
                }
                "sort_asc" => {
                    state.sort_asc = value == "true";
                }
                "window_width" => {
                    window_state.width = value.parse().ok();
                }
                "window_height" => {
                    window_state.height = value.parse().ok();
                }
                "window_x" => {
                    window_state.x = value.parse().ok();
                }
                "window_y" => {
                    window_state.y = value.parse().ok();
                }
                "window_maximized" => {
                    window_state.is_maximized = value == "true";
                }
                "window_fullscreen" => {
                    window_state.is_fullscreen = value == "true";
                }
                "left_sidebar_width" => {
                    state.left_sidebar_width = value.parse().ok();
                }
                "right_sidebar_width" => {
                    state.right_sidebar_width = value.parse().ok();
                }
                "translation_keys" => {
                    if let Ok(keys) = serde_json::from_str::<HashMap<String, String>>(&value) {
                        state.translation_keys = keys;
                    }
                }
                "translation_original_expanded" => {
                    state.translation_original_expanded = value == "true";
                }
                "google_drive_refresh_token" => {
                    state.google_drive_refresh_token = value;
                }
                "webdav_password" => {
                    state.webdav_password = value;
                }
                _ => {}
            }
        }

        state.window_state = window_state;
        debug!("本地状态管理: UI 状态加载完成");
        Ok(state)
    }

    /// 将内存中的状态保存到数据库
    /// 使用事务确保原子性
    pub fn save_all(&self, state: &AppUiState) -> Result<()> {
        debug!("本地状态管理: 正在保存 UI 状态到数据库");
        let mut conn = Connection::open(&self.db_path)?;
        let tx = conn.transaction()?;

        // Helper closure to upsert
        {
            let mut upsert = tx.prepare(
                "INSERT INTO ui_state (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            )?;

            // 1. Expanded Folders
            let expanded_json = serde_json::to_string(&state.expanded_folder_ids)?;
            upsert.execute(params!["expanded_folder_ids", expanded_json])?;

            // 2. Selected Sidebar Item
            if let Some(ref item) = state.selected_sidebar_item {
                upsert.execute(params!["selected_sidebar_item", item])?;
            } else {
                // 如果为 None，可能需要删除？或者存空字符串
                // 这里选择删除，或者忽略
                tx.execute(
                    "DELETE FROM ui_state WHERE key = ?1",
                    params!["selected_sidebar_item"],
                )?;
            }

            // 4. Sort Config
            if let Some(ref field) = state.sort_field {
                upsert.execute(params!["sort_field", field])?;
            }
            upsert.execute(params!["sort_asc", state.sort_asc.to_string()])?;

            // 5. Window State
            if let Some(w) = state.window_state.width {
                upsert.execute(params!["window_width", w.to_string()])?;
            }
            if let Some(h) = state.window_state.height {
                upsert.execute(params!["window_height", h.to_string()])?;
            }
            if let Some(x) = state.window_state.x {
                upsert.execute(params!["window_x", x.to_string()])?;
            }
            if let Some(y) = state.window_state.y {
                upsert.execute(params!["window_y", y.to_string()])?;
            }
            upsert.execute(params![
                "window_maximized",
                state.window_state.is_maximized.to_string()
            ])?;
            upsert.execute(params![
                "window_fullscreen",
                state.window_state.is_fullscreen.to_string()
            ])?;

            if let Some(w) = state.left_sidebar_width {
                upsert.execute(params!["left_sidebar_width", w.to_string()])?;
            }
            if let Some(w) = state.right_sidebar_width {
                upsert.execute(params!["right_sidebar_width", w.to_string()])?;
            }
            let keys_json = serde_json::to_string(&state.translation_keys)?;
            upsert.execute(params!["translation_keys", keys_json])?;
            upsert.execute(params![
                "translation_original_expanded",
                state.translation_original_expanded.to_string()
            ])?;
            upsert.execute(params![
                "google_drive_refresh_token",
                state.google_drive_refresh_token
            ])?;
            upsert.execute(params![
                "webdav_password",
                state.webdav_password
            ])?;
        }

        tx.commit()?;
        debug!("本地状态管理: UI 状态保存完成");
        Ok(())
    }

    /// 从 state.db 加载 config JSON blob
    pub fn load_config(&self) -> Result<Option<String>> {
        debug!("本地状态管理: 正在加载配置");
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare("SELECT value FROM ui_state WHERE key = ?1")?;
        let mut rows = stmt.query(params!["config"])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    /// 将 config JSON blob 保存到 state.db
    pub fn save_config(&self, config_json: &str) -> Result<()> {
        debug!("本地状态管理: 正在保存配置");
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "INSERT INTO ui_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params!["config", config_json],
        )?;
        Ok(())
    }

    pub fn get_pdf_state(&self, id: &str) -> Result<Option<models::local_state::PdfState>> {
        debug!("本地状态管理: 正在加载 PDF 阅读器状态 (ID: {id})");
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT path, page_index, zoom_level, offset_y, fit_to_width,
                    is_left_sidebar_open, is_right_sidebar_open,
                    left_sidebar_width, right_sidebar_width, last_read_at,
                    COALESCE(auto_translate, 1) as auto_translate
             FROM pdf_state WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(models::local_state::PdfState {
                path: row.get(0)?,
                page_index: row.get(1)?,
                zoom_level: row.get(2)?,
                offset_y: row.get(3)?,
                fit_to_width: row.get::<_, i32>(4)? != 0,
                is_left_sidebar_open: row.get::<_, i32>(5)? != 0,
                is_right_sidebar_open: row.get::<_, i32>(6)? != 0,
                left_sidebar_width: row.get(7)?,
                right_sidebar_width: row.get(8)?,
                last_read_at: row.get(9)?,
                auto_translate: row.get::<_, i32>(10)? != 0,
                id: id.to_string(),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn save_pdf_state(
        &self,
        id: &str,
        path: &str,
        page_index: u16,
        zoom_level: f32,
        offset_y: f32,
        fit_to_width: bool,
        is_left_sidebar_open: bool,
        is_right_sidebar_open: bool,
        left_sidebar_width: f32,
        right_sidebar_width: f32,
        auto_translate: bool,
    ) -> Result<()> {
        debug!(
            "本地状态管理: 保存 PDF 阅读器状态 (ID: {id}, page: {page_index}, zoom: {zoom_level})"
        );
        let conn = Connection::open(&self.db_path)?;
        let last_read_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        conn.execute(
            "INSERT INTO pdf_state (
                id, path, page_index, zoom_level, offset_y, fit_to_width,
                is_left_sidebar_open, is_right_sidebar_open,
                left_sidebar_width, right_sidebar_width, last_read_at,
                auto_translate
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
                path = excluded.path,
                page_index = excluded.page_index,
                zoom_level = excluded.zoom_level,
                offset_y = excluded.offset_y,
                fit_to_width = excluded.fit_to_width,
                auto_translate = excluded.auto_translate,
                is_left_sidebar_open = excluded.is_left_sidebar_open,
                is_right_sidebar_open = excluded.is_right_sidebar_open,
                left_sidebar_width = excluded.left_sidebar_width,
                right_sidebar_width = excluded.right_sidebar_width,
                last_read_at = excluded.last_read_at",
            params![
                id,
                path,
                page_index,
                zoom_level,
                offset_y,
                if fit_to_width { 1 } else { 0 },
                if is_left_sidebar_open { 1 } else { 0 },
                if is_right_sidebar_open { 1 } else { 0 },
                left_sidebar_width,
                right_sidebar_width,
                last_read_at,
                if auto_translate { 1 } else { 0 }
            ],
        )?;
        Ok(())
    }
}
