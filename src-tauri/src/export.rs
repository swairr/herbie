//! export 命令的 Rust 侧支撑:切片5 只做「拉当日数据」与「写文件」+ 默认目录。
//!
//! markdown 生成留在 renderer(TS shared `markdown.ts`/`time-markdown.ts`/`journal-markdown.ts`),
//! 故本模块不依赖任何 markdown/聚合逻辑。`assert_day` 与 `write_file` 的安全校验对齐
//! TS `src/main/export-time.ts`/`export-journal.ts` 的 `assertDay` 与 `writeFile` 语义:
//! 校验失败返回 `Err(String)`,renderer 侧统一转 `{ ok: false, error }`。

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{journal::JournalEntry, segment::Segment};

/// 「拉当日数据」的返回值,字段名与 renderer wrapper 的 `ExportDayData` 接口对齐
/// (`segments` / `journal`,两者本身已是 camelCase 序列化)。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDayData {
    pub segments: Vec<Segment>,
    pub journal: Vec<JournalEntry>,
}

/// 校验本地日串必须严格为 `YYYY-MM-DD`(对齐 TS `DAY_RE`)。renderer 只能传这种 day,
/// 同时它也挡住经由 day 构造文件名时可能发生的路径穿越。
pub fn assert_day(day: &str) -> Result<(), String> {
    let b = day.as_bytes();
    let valid = b.len() == 10
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2].is_ascii_digit()
        && b[3].is_ascii_digit()
        && b[4] == b'-'
        && b[5].is_ascii_digit()
        && b[6].is_ascii_digit()
        && b[7] == b'-'
        && b[8].is_ascii_digit()
        && b[9].is_ascii_digit();
    if valid {
        Ok(())
    } else {
        Err(format!("invalid day: {}", day))
    }
}

/// 文件名是否带盘符前缀(`C:foo` 之类;`C:\foo` 已被 `is_absolute` 覆盖)。Windows 下
/// `Path::is_absolute` 对无根盘符相对路径返回 false,故需单独校验。
fn has_drive_prefix(filename: &str) -> bool {
    let b = filename.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// 写文件(utf8),对齐 TS `writeExportFile`/`writeTimeFile`/`writeJournalFile`:
/// 先递归创建父目录,再 `std::fs::write`,返回 join 后的路径字符串。
///
/// 安全校验(纵深防御,renderer 侧 day 已由 `assert_day` 把关):filename 非空、非绝对路径、
/// 不以 `/` 或 `\` 开头、不含盘符前缀、不含 `..` 组件;任一违规返回 Err 且不写文件。
pub fn write_file(dir: &str, filename: &str, content: &str) -> Result<String, String> {
    if filename.is_empty() {
        return Err("invalid filename: empty".to_string());
    }
    if filename.starts_with('/') || filename.starts_with('\\') {
        return Err(format!("invalid filename: leading separator {}", filename));
    }
    if has_drive_prefix(filename) {
        return Err(format!("invalid filename: drive prefix {}", filename));
    }
    let p = Path::new(filename);
    if p.is_absolute() {
        return Err(format!("invalid filename: absolute path {}", filename));
    }
    for comp in p.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            return Err(format!("invalid filename: parent dir component {}", filename));
        }
    }
    let target = Path::new(dir).join(filename);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&target, content.as_bytes()).map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().into_owned())
}

/// 默认导出目录 `%APPDATA%/herbie` —— 与 `db::default_db_path()` 的目录相同,沿用
/// Electron `userData`(即 `%APPDATA%/<appName>`)语义,使切片6/7 打开旧库与导出默认目录
/// 落在同一处。与 db.rs 一致,APPDATA 缺失时 panic(Windows 上必有)。
pub fn default_export_dir() -> String {
    let appdata = std::env::var("APPDATA").expect("APPDATA not set");
    PathBuf::from(appdata).join("herbie").to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    /// 临时目录守卫:唯一随机子目录建在系统 temp 下,测试结束自动整目录清理
    /// (cargo 测试不写仓库目录)。
    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("herbie-export-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
        fn dir_str(&self) -> &str {
            self.0.to_str().unwrap()
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // 对齐 export-time.test.ts 的 writeTimeFile:写 `time/2026-08-03.md`,路径与内容回读一致。
    #[test]
    fn write_time_file_writes_to_time_subdir_and_returns_path() {
        let td = TempDir::new();
        let path = write_file(td.dir_str(), "time/2026-08-03.md", "# hello\n").unwrap();
        let expected = td.path().join("time").join("2026-08-03.md");
        assert_eq!(Path::new(&path), expected.as_path());
        let data = std::fs::read_to_string(expected).unwrap();
        assert_eq!(data, "# hello\n");
    }

    // 对齐 export-time.test.ts 的 writeTimeFile creates nested time dir if missing。
    #[test]
    fn write_time_file_creates_nested_time_dir_if_missing() {
        let td = TempDir::new();
        let path = write_file(td.dir_str(), "time/2026-08-03.md", "x").unwrap();
        assert!(Path::new(&path).ends_with(Path::new("time").join("2026-08-03.md")));
        assert!(td.path().join("time").join("2026-08-03.md").exists());
    }

    // 对齐 export-time/export-journal 的路径穿越拒绝:违规 Err 且不写任何文件。
    #[test]
    fn write_file_rejects_path_traversal() {
        let td = TempDir::new();
        for bad in [
            "../evil",
            "2026-08-03/../..",
            "/abs/path.md",
            "\\abs.md",
            "C:\\evil.md",
            "C:evil.md",
            "\\\\server\\share.md",
            "",
        ] {
            assert!(write_file(td.dir_str(), bad, "x").is_err(), "should reject {bad:?}");
        }
        assert!(
            td.path().read_dir().unwrap().next().is_none(),
            "no file should be written for rejected names"
        );
    }

    // 对齐 export-journal.test.ts 的 writeJournalFile:写 `journal/2026-08-04.md`。
    #[test]
    fn write_journal_file_writes_to_journal_subdir() {
        let td = TempDir::new();
        let path = write_file(td.dir_str(), "journal/2026-08-04.md", "# 日志 2026-08-04\n").unwrap();
        let expected = td.path().join("journal").join("2026-08-04.md");
        assert_eq!(Path::new(&path), expected.as_path());
        let data = std::fs::read_to_string(expected).unwrap();
        assert_eq!(data, "# 日志 2026-08-04\n");
    }

    // 对齐 export-journal.test.ts 的 overwrites an existing file(幂等重导出)。
    #[test]
    fn write_file_overwrites_existing_file() {
        let td = TempDir::new();
        let first = write_file(td.dir_str(), "journal/2026-08-04.md", "# first\n").unwrap();
        let path = write_file(td.dir_str(), "journal/2026-08-04.md", "# second\n").unwrap();
        assert_eq!(Path::new(&path), Path::new(&first));
        let data = std::fs::read_to_string(path).unwrap();
        assert_eq!(data, "# second\n");
    }

    // 对齐 export-time.test.ts 的 buildTimeContent rejects a path-traversal day。
    #[test]
    fn assert_day_accepts_valid_and_rejects_traversal() {
        assert_eq!(assert_day("2026-08-03"), Ok(()));
        assert!(assert_day("not-a-date").is_err());
        assert!(assert_day("../evil").is_err());
        assert!(assert_day("2026-08-03/../..").is_err());
        assert!(assert_day("2026-8-03").is_err());
    }

    // 默认目录 = %APPDATA%/herbie(与 db::default_db_path() 同目录)。
    #[test]
    fn default_export_dir_joins_apdata_herbie() {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let expected = PathBuf::from(appdata).join("herbie");
            assert_eq!(Path::new(&default_export_dir()), expected.as_path());
        }
    }
}
