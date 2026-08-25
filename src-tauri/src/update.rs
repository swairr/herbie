//! 更新子系统:nightly 通道自动检查 + 绿色版自替换(计划见 `.kilo/plans/...md`)。
//!
//! 设计要点:
//! - 版本标识由构建期 `HERBIE_NIGHTLY_SHA` 嵌入;本地/CI 之外的构建为 None → 更新功能
//!   禁用(状态 DevBuild)。
//! - 检查清单固定 `MANIFEST_URL`(nightly release 附带的 `latest.json`),sha 不等即视为
//!   有更新(nightly 单向前进,只做不等比较足够)。
//! - 状态变更统一走 `set_state`:`update://status` 事件 + 托盘菜单项原地刷新。
//! - 替换流程:下载到 exe 同目录 `herbie_new.exe` → sha256 校验 → spawn 分离的 PowerShell
//!   helper(等待旧进程退出 → 重试 Move → Start-Process)→ `app.exit(0)` 复用现有退出清理
//!   路径(lib.rs RunEvent::ExitRequested:关开放段/拆钩子/反注册快捷键)。
//! - 线程模型对齐 idle poll(分片 1s 睡眠 + 全局停止标志 + JoinHandle 持有),见 lib.rs。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

use crate::settings;
use crate::tracker::{ConnAccess, GlobalConn};

/// 检查清单固定地址(nightly prerelease 附带的 `latest.json`,GitHub 直链匿名可访问)。
/// 构建期可用 `HERBIE_UPDATE_MANIFEST_URL` 覆盖(本地端到端联调用)。
pub fn manifest_url() -> &'static str {
    option_env!("HERBIE_UPDATE_MANIFEST_URL")
        .unwrap_or("https://github.com/swairr/herbie/releases/download/nightly/latest.json")
}

/// 当前构建嵌入的 commit SHA。本地开发 / 手工构建未设 `HERBIE_NIGHTLY_SHA` → None。
pub fn build_sha() -> Option<&'static str> {
    option_env!("HERBIE_NIGHTLY_SHA")
}

/// settings 键统一定义在 `settings.rs`(单源,含默认值表与 props 对拍锚点),此处仅引用:
/// `autoUpdateCheck` 由 renderer 设置页开关读写;`updateNotifiedSha` / `lastUpdateCheck`
/// 由本模块自用,默认空串。
pub(crate) use crate::settings::{
    KEY_AUTO_UPDATE_CHECK, KEY_LAST_CHECK, KEY_NOTIFIED_SHA,
};

/// nightly release 附带的 `latest.json`,与 CI `nightly.yml` 生成格式对齐(camelCase)。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateManifest {
    pub sha: String,
    pub built_at: String,
    pub exe_url: String,
    pub exe_sha256: String,
    #[serde(default)]
    #[allow(dead_code)] // 清单契约字段(CI 生成含 notes),当前 UI 暂不展示
    pub notes: Option<String>,
}

/// 更新状态机。`update://status` 事件与托盘菜单项均以本类型为内容源。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum UpdateState {
    Idle,
    DevBuild,
    UpToDate,
    Available { sha: String, built_at: String },
    Downloading { progress: u8 },
    Failed { reason: String },
}

// ---- 纯函数(单测覆盖,见文件尾部 tests) ----

/// 解析 `latest.json` 内容。
pub fn parse_manifest(src: &str) -> Result<UpdateManifest, String> {
    serde_json::from_str(src).map_err(|e| format!("清单解析失败: {e}"))
}

/// 计算 SHA-256 十六进制小写串(纯函数,已知向量单测;生产校验走同源流式 `sha256_stream`,
/// 使大文件校验不整读进内存)。
#[cfg_attr(not(test), allow(dead_code))]
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// 是否有更新:本地 SHA 为 None(开发构建)恒 false;否则做不等比较(nightly 单向前进)。
pub fn is_update_available(local: Option<&str>, remote: &str) -> bool {
    match local {
        Some(l) => l != remote,
        None => false,
    }
}

/// 托盘「更新」菜单项 label;Available 用 builtAt 的日期部分(`nightly-20260825` 形)。
pub fn tray_update_label(state: &UpdateState) -> String {
    match state {
        UpdateState::Idle => "检查更新…".to_string(),
        UpdateState::DevBuild => "更新(开发模式)".to_string(),
        UpdateState::UpToDate => "已是最新版本".to_string(),
        UpdateState::Available { built_at, .. } => {
            let date = if built_at.len() >= 10
                && built_at.as_bytes()[4] == b'-'
                && built_at.as_bytes()[7] == b'-'
            {
                // 仅 ASCII 前缀为 `YYYY-MM-DD` 时才截取;非字符边界(异常 builtAt)回退全串,
                // 避免字节切片 panic(release 为 panic=abort)。
                built_at.get(0..10).unwrap_or(built_at.as_str())
            } else {
                built_at.as_str()
            };
            format!("更新到 nightly-{}", date.replace('-', ""))
        }
        UpdateState::Downloading { progress } => format!("下载更新 {progress}%"),
        UpdateState::Failed { .. } => "更新失败".to_string(),
    }
}

/// 托盘「更新」菜单项是否可点:仅 Available(点击打开主窗口导航设置页,确认后更新)。
pub fn tray_update_enabled(state: &UpdateState) -> bool {
    matches!(state, UpdateState::Available { .. })
}

/// 新 exe 的落盘路径:当前 exe 同目录 `herbie_new.exe`。
pub fn leftover_new_exe_path(exe: &std::path::Path) -> std::path::PathBuf {
    exe.parent()
        .map(|p| p.join("herbie_new.exe"))
        .unwrap_or_else(|| std::path::PathBuf::from("herbie_new.exe"))
}

/// PowerShell 单引号字符串;路径中的 `'` 按 PowerShell 语法翻倍转义。
fn pwsh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// 生成更新助手脚本(单测覆盖):等待旧进程退出 → 移动新 exe(move 失败重试 3×2s)→
/// 启动新 exe。helper 由 `powershell` 直接 spawn,与父进程分离,父进程 `app.exit(0)`
/// 后继续运行。
pub fn build_helper_script(exe: &std::path::Path, new: &std::path::Path, pid: u32) -> String {
    let exe_q = pwsh_quote(&exe.to_string_lossy());
    let new_q = pwsh_quote(&new.to_string_lossy());
    format!(
        "Wait-Process -Id {pid}; \
         $moved=$false; \
         for ($i=0; $i -lt 3; $i++) {{ try {{ Move-Item -Force {new_q} {exe_q} -ErrorAction Stop; $moved=$true; break }} catch {{ Start-Sleep -Seconds 2 }} }}; \
         if (-not $moved -and (Test-Path {new_q})) {{ exit 1 }}; \
         Start-Process {exe_q}"
    )
}

/// 检查结果 → 状态转换(纯逻辑,便于单测;网络/字典访问不在其内)。
fn transition_for(prev: &mut UpdateState, local: Option<&str>, manifest: &UpdateManifest) {
    if local.is_none() {
        *prev = UpdateState::DevBuild;
    } else if is_update_available(local, &manifest.sha) {
        *prev = UpdateState::Available {
            sha: manifest.sha.clone(),
            built_at: manifest.built_at.clone(),
        };
    } else {
        *prev = UpdateState::UpToDate;
    }
}

// ---- 状态持有 + 广播 ----

static UPDATE_STATE: Mutex<UpdateState> = Mutex::new(UpdateState::Idle);
/// 下载/替换进行中标志(AtomicBool 而非 MutexGuard,guard 不 Send,无法整体移入线程)。
static APPLY_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub fn get_state() -> UpdateState {
    UPDATE_STATE.lock().expect("update state mutex poisoned").clone()
}

/// 状态变更的唯出口:写状态 → 广播 `update://status` → 原地刷新托盘菜单项。
fn set_state(app: &AppHandle, state: UpdateState) {
    {
        let mut g = UPDATE_STATE.lock().expect("update state mutex poisoned");
        *g = state;
    }
    let _ = app.emit("update://status", get_state());
    let _ = crate::shell::refresh_tray_update_item();
}

// ---- 检查 ----

fn fetch_manifest() -> Result<UpdateManifest, String> {
    let body = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("构造请求客户端失败: {e}"))?
        .get(manifest_url())
        .send()
        .map_err(|e| format!("获取更新清单失败: {e}"))?
        .text()
        .map_err(|e| format!("读取更新清单失败: {e}"))?;
    parse_manifest(&body)
}

fn auto_update_enabled() -> bool {
    GlobalConn.lock_conn(|conn| settings::get_with_default(conn, KEY_AUTO_UPDATE_CHECK)) == "true"
}

/// 通知去重:新 sha 且不等于 `updateNotifiedSha` 才 toast 一次并落 key,避免每日自动
/// 检查重复打扰(托盘菜单项始终反映待更新状态)。
fn notify_new_version(app: &AppHandle, manifest: &UpdateManifest) {
    let already = GlobalConn.lock_conn(|conn| settings::get_with_default(conn, KEY_NOTIFIED_SHA));
    if already == manifest.sha {
        return;
    }
    let _ = app
        .notification()
        .builder()
        .title("有可用的 Herbie nightly 更新")
        .body(format!("构建于 {},可从设置页或托盘更新", manifest.built_at))
        .show();
    GlobalConn.lock_conn(|conn| {
        let _ = settings::set(conn, KEY_NOTIFIED_SHA, &manifest.sha);
    });
}

/// 执行一轮检查。dev/本地构建(无内嵌 SHA)直接进入 DevBuild,不发起网络请求;
/// 自动检查失败静默(不打扰);手动检查失败写状态供设置页展示。
fn check_once(app: &AppHandle, manual: bool) {
    let local = build_sha();
    if local.is_none() {
        set_state(app, UpdateState::DevBuild);
        return;
    }
    match fetch_manifest() {
        Ok(manifest) => {
            GlobalConn.lock_conn(|conn| {
                let _ = settings::set(
                    conn,
                    KEY_LAST_CHECK,
                    &crate::time::iso_utc_z_millis(chrono::Utc::now()),
                );
            });
            if is_update_available(local, &manifest.sha) {
                notify_new_version(app, &manifest);
            }
            let mut state = get_state();
            transition_for(&mut state, local, &manifest);
            set_state(app, state);
        }
        Err(e) => {
            // 手动检查失败仅写入 Failed,但不得覆盖已确认的 Available(否则托盘/设置页
            // 更新入口一起失效,恢复只能等下一次成功检查——最坏 24h)。
            if manual && !matches!(get_state(), UpdateState::Available { .. }) {
                set_state(app, UpdateState::Failed { reason: e });
            }
        }
    }
}

// ---- Tauri 命令(注册见 lib.rs invoke_handler) ----

/// 触发一次检查(后台线程执行,结果经 `update://status` 事件推送);返回触发时的状态。
#[tauri::command]
pub fn update_check(app: AppHandle) -> Result<UpdateState, String> {
    let app2 = app.clone();
    thread::Builder::new()
        .name("herbie-update-check-manual".to_string())
        .spawn(move || check_once(&app2, true))
        .map_err(|e| format!("启动检查线程失败: {e}"))?;
    Ok(get_state())
}

/// 读取当前状态(renderer 设置页初始化 / 托盘刷新用)。
#[tauri::command]
pub fn update_get_state() -> Result<UpdateState, String> {
    Ok(get_state())
}

/// 当前构建信息:嵌入 SHA / 是否开发模式 / 最近一次检查。dev 构建无 SHA,更新入口禁用。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateVersion {
    pub sha: Option<String>,
    pub dev_build: bool,
    pub last_check: Option<String>,
}

#[tauri::command]
pub fn update_get_version() -> Result<UpdateVersion, String> {
    let last_check = GlobalConn.lock_conn(|conn| settings::get(conn, KEY_LAST_CHECK));
    Ok(UpdateVersion {
        sha: build_sha().map(str::to_string),
        dev_build: build_sha().is_none(),
        last_check,
    })
}

/// 应用更新:下载 → 校验 → spawn 分离 helper → `app.exit(0)` 复用退出清理路径。
#[tauri::command]
pub fn update_apply(app: AppHandle) -> Result<(), String> {
    match get_state() {
        UpdateState::DevBuild => return Err("当前为开发模式,不支持更新".into()),
        UpdateState::Available { .. } => {}
        UpdateState::Downloading { .. } => return Err("正在下载更新,请稍候".into()),
        _ => return Err("当前没有可应用的更新".into()),
    }
    if APPLY_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return Err("更新正在进行,请稍候".into());
    }
    let spawned = thread::Builder::new()
        .name("herbie-update-apply".to_string())
        .spawn(move || {
            // 仅失败/已最新路径会返回(成功路径 `app.exit(0)` 直接退出进程,无需复位);
            // 复位使下次启动同一轮内可再次尝试。(进程退出时该语句不执行,无实际影响。)
            apply_inner(&app);
            APPLY_IN_PROGRESS.store(false, Ordering::SeqCst);
        });
    match spawned {
        Ok(_) => Ok(()),
        Err(e) => {
            // spawn 失败也要复位标志,否则本进程内更新子系统永久卡死。
            APPLY_IN_PROGRESS.store(false, Ordering::SeqCst);
            Err(format!("启动更新线程失败: {e}"))
        }
    }
}

fn apply_inner(app: &AppHandle) {
    let manifest = match fetch_manifest() {
        Ok(m) => m,
        Err(e) => {
            set_state(app, UpdateState::Failed { reason: e });
            return;
        }
    };
    if build_sha().is_none() {
        set_state(app, UpdateState::DevBuild);
        return;
    }
    if !is_update_available(build_sha(), &manifest.sha) {
        set_state(app, UpdateState::UpToDate);
        return;
    }
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            set_state(app, UpdateState::Failed { reason: format!("无法定位当前程序: {e}") });
            return;
        }
    };
    let new_exe = leftover_new_exe_path(&exe);
    set_state(app, UpdateState::Downloading { progress: 0 });
    // 独占创建下载目标后持同一句柄写入并校验:路径上预置的同名文件(本地替换攻击面)
    // 不会被覆盖,校验与执行之间不重新按路径打开文件。
    let mut file = match create_download_file(&new_exe) {
        Ok(f) => f,
        Err(e) => {
            set_state(app, UpdateState::Failed { reason: format!("无法创建下载文件: {e}") });
            return;
        }
    };
    if let Err(e) = download_exe(app, &manifest, &mut file) {
        drop(file);
        let _ = std::fs::remove_file(&new_exe);
        set_state(app, UpdateState::Failed { reason: e });
        return;
    }
    use std::io::{Seek, SeekFrom, Write};
    if let Err(e) = file.flush() {
        drop(file);
        let _ = std::fs::remove_file(&new_exe);
        set_state(app, UpdateState::Failed { reason: format!("写入临时文件失败: {e}") });
        return;
    }
    if let Err(e) = file.seek(SeekFrom::Start(0)) {
        drop(file);
        let _ = std::fs::remove_file(&new_exe);
        set_state(app, UpdateState::Failed { reason: format!("读取下载文件失败: {e}") });
        return;
    }
    let digest = match sha256_stream(&mut file) {
        Ok(d) => d,
        Err(e) => {
            drop(file);
            let _ = std::fs::remove_file(&new_exe);
            set_state(app, UpdateState::Failed { reason: format!("读取下载文件失败: {e}") });
            return;
        }
    };
    drop(file);
    if !digest.eq_ignore_ascii_case(&manifest.exe_sha256) {
        let _ = std::fs::remove_file(&new_exe);
        set_state(app, UpdateState::Failed {
            reason: "下载内容校验不通过,已删除临时文件".into(),
        });
        return;
    }
    let script = build_helper_script(&exe, &new_exe, std::process::id());
    let spawned = std::process::Command::new("powershell")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-WindowStyle")
        .arg("Hidden")
        .arg("-Command")
        .arg(script)
        .spawn();
    if spawned.is_err() {
        let _ = std::fs::remove_file(&new_exe);
        set_state(app, UpdateState::Failed { reason: "启动更新助手失败".into() });
        return;
    }
    // helper 已分离,等待本进程退出后移动并重启旧 exe 文件;此处走现有唯一退出路径
    // (RunEvent::ExitRequested → 关开放段/拆钩子/反注册快捷键,与托盘退出一致)。
    app.exit(0);
}

/// 分块下载新 exe 到已独占创建的句柄,按 content_length 推进进度;进程内不重复打开
/// 路径(校验沿用同一句柄,见 `apply_inner`)。
fn download_exe(
    app: &AppHandle,
    manifest: &UpdateManifest,
    target: &mut std::fs::File,
) -> Result<(), String> {
    use std::io::{Read, Write};
    let mut resp = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| format!("构造下载客户端失败: {e}"))?
        .get(&manifest.exe_url)
        .send()
        .map_err(|e| format!("下载失败: {e}"))?;
    let total = resp.content_length().unwrap_or(0);
    let mut buf = [0u8; 64 * 1024];
    let mut written: u64 = 0;
    let mut last_pct: Option<u8> = None;
    loop {
        let n = resp.read(&mut buf).map_err(|e| format!("下载中断: {e}"))?;
        if n == 0 {
            break;
        }
        target.write_all(&buf[..n]).map_err(|e| format!("写入临时文件失败: {e}"))?;
        written += n as u64;
        // 仅在百分比变化时广播:避免每个 64KB 块都触发事件 + 托盘菜单 Win32 变更。
        if total > 0 {
            let pct = ((written as f64 / total as f64) * 100.0) as u8;
            let pct = pct.min(99);
            if last_pct != Some(pct) {
                last_pct = Some(pct);
                set_state(app, UpdateState::Downloading { progress: pct });
            }
        }
    }
    target.flush().map_err(|e| format!("写入临时文件失败: {e}"))?;
    Ok(())
}

/// 独占创建下载目标(`create_new`):`herbie_new.exe` 路径上如存在别的进程预置的同名
/// 文件(本地替换攻击面),写入会失败而非覆盖;既有文件为本进程上次崩溃残留时清掉重试。
fn create_download_file(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    use std::fs::OpenOptions;
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(f) => Ok(f),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = std::fs::remove_file(path);
            OpenOptions::new().write(true).create_new(true).open(path)
        }
        Err(e) => Err(e),
    }
}

/// 流式计算 SHA-256 十六进制小写串:不把整个文件读进内存(新 exe 可达数十 MB)。
fn sha256_stream(mut r: impl std::io::Read) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

// ---- 自动检查线程(仿 idle poll 线程模式,分片睡眠响应退出标志) ----

static UPDATE_CHECK_STOP: AtomicBool = AtomicBool::new(false);
static UPDATE_CHECK_THREAD: Mutex<Option<thread::JoinHandle<()>>> = Mutex::new(None);

/// 清理上次更新失败残留的 `herbie_new.exe`(启动时调用)。
pub fn cleanup_leftover_new_exe() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(leftover_new_exe_path(&exe));
    }
}

/// 启动自动更新检查线程:启动后 30s 首查,之后每 24h;分片睡眠(1s 粒度)响应
/// `stop_update_check_system`。每轮先读 `autoUpdateCheck`,关闭则跳过。
/// 仅在非 dev 构建(内嵌 SHA)时由 setup 调用。
pub fn start_update_check_system(app: &AppHandle) {
    cleanup_leftover_new_exe();
    if UPDATE_CHECK_THREAD
        .lock()
        .expect("update check mutex poisoned")
        .is_some()
    {
        return;
    }
    let app = app.clone();
    let handle = thread::Builder::new()
        .name("herbie-update-check".to_string())
        .spawn(move || loop {
            for _ in 0..30 {
                thread::sleep(Duration::from_millis(1000));
                if UPDATE_CHECK_STOP.load(Ordering::SeqCst) {
                    return;
                }
            }
            if auto_update_enabled() {
                check_once(&app, false);
            }
            for _ in 0..(24 * 60 * 60) {
                thread::sleep(Duration::from_millis(1000));
                if UPDATE_CHECK_STOP.load(Ordering::SeqCst) {
                    return;
                }
            }
        })
        .expect("failed to spawn update check thread");
    *UPDATE_CHECK_THREAD.lock().expect("update check mutex poisoned") = Some(handle);
}

/// 停止自动检查线程(`RunEvent::ExitRequested` 中调用):先置标志,再做**有界** join ——
/// 检查线程只在分片睡眠边界退出,若恰好处于网络请求中(fetch_manifest 最长 15s),
/// 无界 join 会让退出阻塞最多 15s,表现为「关闭无响应」。有界等待后再保留的线程随
/// 进程退出回收(WAL 语义与托盘退出一致)。
pub fn stop_update_check_system() {
    use std::time::Instant;
    UPDATE_CHECK_STOP.store(true, Ordering::SeqCst);
    let Some(join) = UPDATE_CHECK_THREAD.lock().expect("update check mutex poisoned").take() else {
        return;
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    while !join.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));
    }
    if join.is_finished() {
        let _ = join.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(sha: &str) -> UpdateManifest {
        UpdateManifest {
            sha: sha.to_string(),
            built_at: "2026-08-25T03:00:00+08:00".to_string(),
            exe_url: "https://example.com/herbie.exe".to_string(),
            exe_sha256: "abcd".to_string(),
            notes: None,
        }
    }

    #[test]
    fn parse_manifest_full_fieldset() {
        let m = parse_manifest(
            r#"{"sha":"abc123","builtAt":"2026-08-25T03:00:00+08:00","exeUrl":"https://example.com/herbie.exe","exeSha256":"abcd","notes":"nightly"}"#,
        )
        .unwrap();
        assert_eq!(m.sha, "abc123");
        assert_eq!(m.built_at, "2026-08-25T03:00:00+08:00");
        assert_eq!(m.exe_url, "https://example.com/herbie.exe");
        assert_eq!(m.exe_sha256, "abcd");
        assert_eq!(m.notes.as_deref(), Some("nightly"));
    }

    #[test]
    fn parse_manifest_optional_notes_absent_ok() {
        let m = parse_manifest(
            r#"{"sha":"abc","builtAt":"2026-08-25T03:00:00+08:00","exeUrl":"u","exeSha256":"h"}"#,
        )
        .unwrap();
        assert!(m.notes.is_none());
    }

    #[test]
    fn parse_manifest_rejects_missing_and_invalid() {
        assert!(parse_manifest(r#"{"sha":"x"}"#).is_err());
        assert!(parse_manifest("not json").is_err());
    }

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn update_availability_rules() {
        assert!(!is_update_available(None, "abc"));
        assert!(!is_update_available(Some("abc"), "abc"));
        assert!(is_update_available(Some("abc"), "def"));
    }

    #[test]
    fn tray_labels_and_enabled_per_state() {
        assert_eq!(tray_update_label(&UpdateState::Idle), "检查更新…");
        assert_eq!(tray_update_label(&UpdateState::DevBuild), "更新(开发模式)");
        assert_eq!(tray_update_label(&UpdateState::UpToDate), "已是最新版本");
        assert_eq!(
            tray_update_label(&UpdateState::Available {
                sha: "a".into(),
                built_at: "2026-08-25T03:00:00+08:00".into(),
            }),
            "更新到 nightly-20260825"
        );
        assert_eq!(
            tray_update_label(&UpdateState::Downloading { progress: 42 }),
            "下载更新 42%"
        );
        assert_eq!(tray_update_label(&UpdateState::Failed { reason: "x".into() }), "更新失败");

        assert!(tray_update_enabled(&UpdateState::Available {
            sha: "a".into(),
            built_at: "2026-08-25".into(),
        }));
        assert!(!tray_update_enabled(&UpdateState::Idle));
        assert!(!tray_update_enabled(&UpdateState::Downloading { progress: 10 }));
        assert!(!tray_update_enabled(&UpdateState::DevBuild));
    }

    #[test]
    fn leftover_new_exe_path_computed_next_to_exe() {
        let p = leftover_new_exe_path(&std::path::Path::new("C:\\Tools\\herbie.exe"));
        assert_eq!(p, std::path::PathBuf::from("C:\\Tools\\herbie_new.exe"));
        let fallback = leftover_new_exe_path(std::path::Path::new("herbie.exe"));
        assert_eq!(fallback, std::path::PathBuf::from("herbie_new.exe"));
    }

    #[test]
    fn helper_script_quotes_paths_and_retries_move() {
        let exe = std::path::Path::new("C:\\Users\\O'Brien\\herbie.exe");
        let new = std::path::Path::new("C:\\Users\\O'Brien\\herbie_new.exe");
        let script = build_helper_script(exe, new, 1234);
        assert!(script.contains("Wait-Process -Id 1234"));
        // 单引号在 PowerShell 串内翻倍,防止用户目录含撇号时截断命令。
        assert!(script.contains("'C:\\Users\\O''Brien\\herbie_new.exe'"));
        assert!(script.contains("Start-Process 'C:\\Users\\O''Brien\\herbie.exe'"));
        assert!(script.contains("for ($i=0; $i -lt 3"));
    }

    #[test]
    fn transition_rules_map_to_states() {
        let m = manifest("new");
        let mut s = UpdateState::Idle;
        transition_for(&mut s, None, &m);
        assert!(matches!(s, UpdateState::DevBuild));

        transition_for(&mut s, Some("same"), &m);
        match &s {
            UpdateState::Available { sha, built_at } => {
                assert_eq!(sha, "new");
                assert_eq!(built_at, "2026-08-25T03:00:00+08:00");
            }
            other => panic!("expected Available, got {other:?}"),
        }

        transition_for(&mut s, Some("new"), &m);
        assert!(matches!(s, UpdateState::UpToDate));
    }

    #[test]
    fn sha256_stream_matches_known_vector() {
        // 与 `sha256_hex` 已知向量同源:流式实现(生产校验路径)输出应一致。
        let got = sha256_stream(std::io::Cursor::new(b"abc")).unwrap();
        assert_eq!(got, sha256_hex(b"abc"));
        assert_eq!(
            got,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let empty = sha256_stream(std::io::Cursor::new(Vec::<u8>::new())).unwrap();
        assert_eq!(empty, sha256_hex(b""));
    }

    /// 跨语言契约对拍:与 `tests/update-state-fixture.test.ts` 读同一份
    /// `tests/fixtures/update-state.json`。任一侧改动 `UpdateState`/`UpdateVersion`
    /// 的序列化形状或 settings 键名,都必须同步该夹具,否则对应侧用例报警。
    #[test]
    fn serialized_ipc_shapes_match_shared_fixture() {
        use serde_json::Value;

        let raw = include_str!("../../tests/fixtures/update-state.json");
        let fx: Value = serde_json::from_str(raw).expect("update-state.json must parse");

        let cases: Vec<(UpdateState, &str)> = vec![
            (UpdateState::Idle, "idle"),
            (UpdateState::DevBuild, "devBuild"),
            (UpdateState::UpToDate, "upToDate"),
            (
                UpdateState::Available {
                    sha: "abc123".into(),
                    built_at: "2026-08-25T03:00:00+08:00".into(),
                },
                "available",
            ),
            (UpdateState::Downloading { progress: 42 }, "downloading"),
            (UpdateState::Failed { reason: "网络错误".into() }, "failed"),
        ];
        for (state, tag) in cases {
            let actual: Value = serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
            let expected: Value =
                serde_json::from_str(fx["states"][tag].as_str().unwrap()).unwrap();
            assert_eq!(actual, expected, "UpdateState::{tag} 与夹具不一致");
        }

        let version = UpdateVersion {
            sha: Some("abc123".into()),
            dev_build: false,
            last_check: Some("2026-08-25T03:00:00.000Z".into()),
        };
        let actual: Value = serde_json::from_str(&serde_json::to_string(&version).unwrap()).unwrap();
        let expected: Value = serde_json::from_str(fx["version"].as_str().unwrap()).unwrap();
        assert_eq!(actual, expected, "UpdateVersion 与夹具不一致");

        let keys = fx["settingsKeys"].as_array().unwrap();
        for k in [KEY_AUTO_UPDATE_CHECK, KEY_NOTIFIED_SHA, KEY_LAST_CHECK] {
            assert!(
                keys.iter().any(|v| v.as_str() == Some(k)),
                "settings 键 {k} 不在夹具 settingsKeys 中"
            );
        }
    }
}