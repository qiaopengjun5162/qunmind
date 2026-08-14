use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::config::PublicSourcesConfig;
use crate::error::{QunMindError, Result};

#[derive(Debug, Clone)]
pub struct WechatArticleUrlOutput {
    pub url: String,
    pub helper_bin: String,
    pub helper_kind: String,
    pub output_dir: PathBuf,
    pub run_dir: PathBuf,
    pub article_dir: Option<PathBuf>,
    pub markdown_path: Option<PathBuf>,
    pub images_dir: Option<PathBuf>,
    pub parsed: Option<WechatArticleMarkdown>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WechatArticleMarkdown {
    pub title: String,
    pub account_name: Option<String>,
    pub published_at: Option<String>,
    pub source_url: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WechatArticleUrlFailure {
    pub requested_url: Option<String>,
    pub normalized_url: Option<String>,
    pub helper_bin: String,
    pub helper_kind: String,
    pub output_dir: Option<PathBuf>,
    pub run_dir: Option<PathBuf>,
    pub failure_stage: &'static str,
    pub message: String,
    pub stdout_excerpt: Option<String>,
    pub stderr_excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HelperBinaryStatus {
    resolved_path: Option<PathBuf>,
    exists: bool,
    executable: bool,
}

#[derive(Debug, Clone, Copy)]
struct WechatArticleDoctorContext<'a> {
    helper_kind: WechatArticleHelperKind,
    helper_bin: &'a str,
    helper_status: &'a HelperBinaryStatus,
    output_dir_declared: bool,
    output_dir_exists: bool,
    output_dir_parent_exists: bool,
    requested_url: Option<&'a str>,
    normalized_url: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WechatArticleHelperKind {
    Jackwener,
    Noisepoint,
    Generic,
}

impl WechatArticleHelperKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Jackwener => "jackwener",
            Self::Noisepoint => "noisepoint",
            Self::Generic => "generic",
        }
    }
}

impl fmt::Display for WechatArticleUrlFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.failure_stage, self.message)
    }
}

impl std::error::Error for WechatArticleUrlFailure {}

pub fn run_wechat_article_url_helper(
    public_sources: &PublicSourcesConfig,
    url: &str,
    output_dir: Option<&Path>,
) -> std::result::Result<WechatArticleUrlOutput, Box<WechatArticleUrlFailure>> {
    let helper_bin = public_sources.wechat_article_helper_bin.trim();
    if helper_bin.is_empty() {
        return Err(Box::new(WechatArticleUrlFailure {
            requested_url: Some(url.trim().to_string()),
            normalized_url: None,
            helper_bin: helper_bin.to_string(),
            helper_kind: detect_helper_kind(helper_bin).as_str().to_string(),
            output_dir: output_dir.map(Path::to_path_buf),
            run_dir: None,
            failure_stage: "config_validation",
            message: "未配置 public_sources.wechat_article_helper_bin；请先显式配置单篇公众号链接 helper，再使用 wechat-article-url 入口。".to_string(),
            stdout_excerpt: None,
            stderr_excerpt: None,
        }));
    }

    let helper_kind = detect_helper_kind(helper_bin);
    let normalized_url = normalize_wechat_article_url(url).map_err(|err| {
        Box::new(WechatArticleUrlFailure {
            requested_url: Some(url.trim().to_string()),
            normalized_url: None,
            helper_bin: helper_bin.to_string(),
            helper_kind: helper_kind.as_str().to_string(),
            output_dir: output_dir.map(Path::to_path_buf),
            run_dir: None,
            failure_stage: "url_validation",
            message: err.to_string(),
            stdout_excerpt: None,
            stderr_excerpt: None,
        })
    })?;
    let output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(public_sources.wechat_article_helper_output_dir.trim()));

    if output_dir.as_os_str().is_empty() {
        return Err(Box::new(WechatArticleUrlFailure {
            requested_url: Some(url.trim().to_string()),
            normalized_url: Some(normalized_url),
            helper_bin: helper_bin.to_string(),
            helper_kind: helper_kind.as_str().to_string(),
            output_dir: Some(output_dir),
            run_dir: None,
            failure_stage: "config_validation",
            message:
                "未配置 public_sources.wechat_article_helper_output_dir，且本次也没有显式传 output_dir。"
                    .to_string(),
            stdout_excerpt: None,
            stderr_excerpt: None,
        }));
    }

    std::fs::create_dir_all(&output_dir).map_err(|err| {
        Box::new(WechatArticleUrlFailure {
            requested_url: Some(url.trim().to_string()),
            normalized_url: Some(normalized_url.clone()),
            helper_bin: helper_bin.to_string(),
            helper_kind: helper_kind.as_str().to_string(),
            output_dir: Some(output_dir.clone()),
            run_dir: None,
            failure_stage: "prepare_output_dir",
            message: format!(
                "创建公众号单链接 helper 输出目录失败 {}: {}",
                output_dir.display(),
                err
            ),
            stdout_excerpt: None,
            stderr_excerpt: None,
        })
    })?;

    let run_dir = output_dir.join(format!("run-{}", current_unix_timestamp_millis()));
    std::fs::create_dir_all(&run_dir).map_err(|err| {
        Box::new(WechatArticleUrlFailure {
            requested_url: Some(url.trim().to_string()),
            normalized_url: Some(normalized_url.clone()),
            helper_bin: helper_bin.to_string(),
            helper_kind: helper_kind.as_str().to_string(),
            output_dir: Some(output_dir.clone()),
            run_dir: Some(run_dir.clone()),
            failure_stage: "prepare_run_dir",
            message: format!(
                "创建公众号单链接 helper 运行目录失败 {}: {}",
                run_dir.display(),
                err
            ),
            stdout_excerpt: None,
            stderr_excerpt: None,
        })
    })?;

    let mut command = Command::new(helper_bin);
    command.args(helper_args_for(helper_kind, &normalized_url, &run_dir));
    if matches!(
        helper_kind,
        WechatArticleHelperKind::Noisepoint | WechatArticleHelperKind::Jackwener
    ) {
        command.current_dir(&run_dir);
    }
    let output = command.output().map_err(|err| {
        Box::new(WechatArticleUrlFailure {
            requested_url: Some(url.trim().to_string()),
            normalized_url: Some(normalized_url.clone()),
            helper_bin: helper_bin.to_string(),
            helper_kind: helper_kind.as_str().to_string(),
            output_dir: Some(output_dir.clone()),
            run_dir: Some(run_dir.clone()),
            failure_stage: "launch_helper",
            message: format!(
                "调用公众号单链接 helper 失败：{}。请确认 {} 已安装且可执行。",
                err, helper_bin
            ),
            stdout_excerpt: None,
            stderr_excerpt: None,
        })
    })?;

    if !output.status.success() {
        let stderr = non_empty_excerpt(String::from_utf8_lossy(&output.stderr).trim());
        let stdout = non_empty_excerpt(String::from_utf8_lossy(&output.stdout).trim());
        let detail = stderr
            .as_deref()
            .or(stdout.as_deref())
            .map(str::to_string)
            .unwrap_or_else(|| format!("exit status {}", output.status));
        return Err(Box::new(WechatArticleUrlFailure {
            requested_url: Some(url.trim().to_string()),
            normalized_url: Some(normalized_url),
            helper_bin: helper_bin.to_string(),
            helper_kind: helper_kind.as_str().to_string(),
            output_dir: Some(output_dir),
            run_dir: Some(run_dir),
            failure_stage: "helper_exit_non_zero",
            message: format!("公众号单链接 helper 执行失败: {detail}"),
            stdout_excerpt: stdout,
            stderr_excerpt: stderr,
        }));
    }

    let (article_dir, markdown_path, images_dir) = discover_helper_output(&run_dir);
    if markdown_path.is_none() {
        return Err(Box::new(WechatArticleUrlFailure {
            requested_url: Some(url.trim().to_string()),
            normalized_url: Some(normalized_url),
            helper_bin: helper_bin.to_string(),
            helper_kind: helper_kind.as_str().to_string(),
            output_dir: Some(output_dir),
            run_dir: Some(run_dir),
            failure_stage: "discover_output",
            message:
                "helper 已成功退出，但运行目录下未发现 markdown 结果；请先查看 doctor 输出的布局预期是否与当前 helper 一致。"
                    .to_string(),
            stdout_excerpt: non_empty_excerpt(String::from_utf8_lossy(&output.stdout).trim()),
            stderr_excerpt: non_empty_excerpt(String::from_utf8_lossy(&output.stderr).trim()),
        }));
    }
    let parsed = markdown_path
        .as_ref()
        .and_then(|path| parse_wechat_article_markdown(path).ok());

    Ok(WechatArticleUrlOutput {
        url: normalized_url,
        helper_bin: helper_bin.to_string(),
        helper_kind: helper_kind.as_str().to_string(),
        output_dir,
        run_dir,
        article_dir,
        markdown_path,
        images_dir,
        parsed,
    })
}

pub fn wechat_article_url_response_json(output: &WechatArticleUrlOutput) -> serde_json::Value {
    let parsed = output.parsed.as_ref().map(|parsed| {
        json!({
            "title": parsed.title,
            "account_name": parsed.account_name,
            "published_at": parsed.published_at,
            "source_url": parsed.source_url,
            "summary": parsed.summary,
        })
    });

    json!({
        "ok": true,
        "url": output.url,
        "helper_bin": output.helper_bin,
        "helper_kind": output.helper_kind,
        "output_dir": output.output_dir.display().to_string(),
        "run_dir": output.run_dir.display().to_string(),
        "article_dir": output.article_dir.as_ref().map(|path| path.display().to_string()),
        "markdown_path": output.markdown_path.as_ref().map(|path| path.display().to_string()),
        "images_dir": output.images_dir.as_ref().map(|path| path.display().to_string()),
        "parsed": parsed,
        "title": output.parsed.as_ref().map(|parsed| parsed.title.clone()),
        "account_name": output.parsed.as_ref().and_then(|parsed| parsed.account_name.clone()),
        "published_at": output.parsed.as_ref().and_then(|parsed| parsed.published_at.clone()),
        "source_url": output.parsed.as_ref().and_then(|parsed| parsed.source_url.clone()),
        "summary": output.parsed.as_ref().and_then(|parsed| parsed.summary.clone()),
        "note": "该入口调用外部 helper 处理单篇公众号链接；失败不会影响 RSS / 日报主链路。",
    })
}

pub fn wechat_article_url_failure_json(failure: &WechatArticleUrlFailure) -> serde_json::Value {
    let doctor_output_dir = failure.output_dir.as_deref();
    let doctor_json = wechat_article_url_doctor_json(
        &PublicSourcesConfig {
            wechat_article_helper_bin: failure.helper_bin.clone(),
            wechat_article_helper_output_dir: failure
                .output_dir
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            ..PublicSourcesConfig::default()
        },
        failure.requested_url.as_deref(),
        doctor_output_dir,
    );

    json!({
        "ok": false,
        "requested_url": failure.requested_url,
        "url": failure.normalized_url,
        "helper_bin": failure.helper_bin,
        "helper_kind": failure.helper_kind,
        "output_dir": failure.output_dir.as_ref().map(|path| path.display().to_string()),
        "run_dir": failure.run_dir.as_ref().map(|path| path.display().to_string()),
        "failure_stage": failure.failure_stage,
        "message": failure.message,
        "stdout_excerpt": failure.stdout_excerpt,
        "stderr_excerpt": failure.stderr_excerpt,
        "recommended_doctor_command": recommended_doctor_command(
            failure.requested_url.as_deref(),
            failure.output_dir.as_deref(),
        ),
        "layout_expectation": helper_layout_expectation(detect_helper_kind(&failure.helper_bin)),
        "helper_known_advice": helper_known_advice(detect_helper_kind(&failure.helper_bin)),
        "doctor": doctor_json,
        "note": "该入口调用外部 helper 处理单篇公众号链接；失败不会影响 RSS / 日报主链路。",
    })
}

pub fn wechat_article_url_doctor_json(
    public_sources: &PublicSourcesConfig,
    url: Option<&str>,
    output_dir: Option<&Path>,
) -> serde_json::Value {
    let helper_bin = public_sources.wechat_article_helper_bin.trim().to_string();
    let helper_kind = detect_helper_kind(helper_bin.as_str());
    let helper_status = helper_binary_status(helper_bin.as_str());
    let normalized_url = url.and_then(|value| normalize_wechat_article_url(value).ok());
    let effective_output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(public_sources.wechat_article_helper_output_dir.trim()));
    let run_dir_example = effective_output_dir.join("run-<timestamp>");
    let output_dir_exists = effective_output_dir.is_dir();
    let output_dir_parent = effective_output_dir.parent().map(Path::to_path_buf);
    let output_dir_parent_exists = output_dir_parent
        .as_ref()
        .is_some_and(|parent| parent.is_dir());
    let output_dir_declared = !effective_output_dir.as_os_str().is_empty();

    let mut blockers = Vec::new();
    if helper_bin.trim().is_empty() {
        blockers.push("helper_bin_missing");
    }
    if !helper_bin.trim().is_empty() && !helper_status.exists {
        blockers.push("helper_bin_not_found");
    }
    if helper_status.exists && !helper_status.executable {
        blockers.push("helper_bin_not_executable");
    }
    if !output_dir_declared {
        blockers.push("output_dir_missing");
    } else if !output_dir_exists && !output_dir_parent_exists {
        blockers.push("output_dir_parent_missing");
    }
    if let Some(original_url) = url
        && normalize_wechat_article_url(original_url).is_err()
    {
        blockers.push("url_invalid");
    }

    let next_steps = wechat_article_url_doctor_next_steps(WechatArticleDoctorContext {
        helper_kind,
        helper_bin: helper_bin.as_str(),
        helper_status: &helper_status,
        output_dir_declared,
        output_dir_exists,
        output_dir_parent_exists,
        requested_url: url,
        normalized_url: normalized_url.as_deref(),
    });

    json!({
        "ok": blockers.is_empty(),
        "doctor": true,
        "url": normalized_url,
        "helper": {
            "configured": !helper_bin.trim().is_empty(),
            "bin": helper_bin,
            "kind": helper_kind.as_str(),
            "resolved_path": helper_status.resolved_path.as_ref().map(|path| path.display().to_string()),
            "exists": helper_status.exists,
            "executable": helper_status.executable,
            "known_advice": helper_known_advice(helper_kind),
            "invocation_preview": {
                "cwd": helper_invocation_cwd(helper_kind, &run_dir_example).display().to_string(),
                "args": helper_args_for(
                    helper_kind,
                    normalized_url.as_deref().unwrap_or("https://mp.weixin.qq.com/s/example"),
                    &run_dir_example,
                ),
            },
            "layout_expectation": helper_layout_expectation(helper_kind),
        },
        "output": {
            "effective_output_dir": effective_output_dir.display().to_string(),
            "exists": output_dir_exists,
            "parent_dir": output_dir_parent.as_ref().map(|path| path.display().to_string()),
            "parent_exists": output_dir_parent_exists,
            "run_dir_example": run_dir_example.display().to_string(),
        },
        "safety": {
            "read_only_diagnostic": true,
            "executes_helper": false,
            "mutates_proxy": false,
            "touches_wechat_ui": false,
            "note": "只做预检和参数预览，不抓文章、不改代理、不碰微信登录态。",
        },
        "blockers": blockers,
        "next_steps": next_steps,
    })
}

fn normalize_wechat_article_url(url: &str) -> Result<String> {
    let trimmed = url.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() {
        return Err(QunMindError::Config("url 不能为空".to_string()));
    }
    if !trimmed.starts_with("https://mp.weixin.qq.com/") {
        return Err(QunMindError::Config(
            "只支持 https://mp.weixin.qq.com/ 单篇文章链接".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn detect_helper_kind(helper_bin: &str) -> WechatArticleHelperKind {
    let lower = Path::new(helper_bin)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or(helper_bin)
        .to_ascii_lowercase();

    if lower.contains("mp-weixin-to-md") {
        WechatArticleHelperKind::Noisepoint
    } else if lower.contains("wechat-article-to-markdown") || lower == "main.py" {
        WechatArticleHelperKind::Jackwener
    } else {
        WechatArticleHelperKind::Generic
    }
}

fn helper_args_for(helper_kind: WechatArticleHelperKind, url: &str, run_dir: &Path) -> Vec<String> {
    match helper_kind {
        WechatArticleHelperKind::Noisepoint => vec![
            url.to_string(),
            "-o".to_string(),
            "article.md".to_string(),
            "--download-assets".to_string(),
            "--assets-dir".to_string(),
            "images".to_string(),
        ],
        WechatArticleHelperKind::Jackwener => vec![url.to_string()],
        WechatArticleHelperKind::Generic => vec![
            url.to_string(),
            "--output".to_string(),
            run_dir.display().to_string(),
        ],
    }
}

fn helper_invocation_cwd(helper_kind: WechatArticleHelperKind, run_dir: &Path) -> &Path {
    if matches!(
        helper_kind,
        WechatArticleHelperKind::Noisepoint | WechatArticleHelperKind::Jackwener
    ) {
        run_dir
    } else {
        Path::new(".")
    }
}

fn helper_layout_expectation(helper_kind: WechatArticleHelperKind) -> &'static str {
    match helper_kind {
        WechatArticleHelperKind::Noisepoint => "flat_markdown_with_optional_images_dir",
        WechatArticleHelperKind::Jackwener => "nested_article_directory_with_markdown_and_images",
        WechatArticleHelperKind::Generic => {
            "generic_output_dir_with_markdown_somewhere_under_run_dir"
        }
    }
}

fn helper_known_advice(helper_kind: WechatArticleHelperKind) -> Vec<&'static str> {
    match helper_kind {
        WechatArticleHelperKind::Noisepoint => vec![
            "默认会把 markdown 直接写进本次 run_dir，而不是再套一层文章目录。",
            "当前适配会默认附带 --download-assets，并把图片放到 run_dir/images。",
            "如果执行成功却没找到结果，优先先看 run_dir 下是否生成了 article.md 或 images/。",
        ],
        WechatArticleHelperKind::Jackwener => vec![
            "常见输出是 run_dir 下再生成一层文章目录，markdown 往往不在根目录。",
            "如果 helper 自身成功退出但结果缺失，优先检查 output/ 或文章标题目录是否真的生成。",
            "这类 helper 更适合作为重备选，不要把它的目录假设扩散到主命令入口。",
        ],
        WechatArticleHelperKind::Generic => vec![
            "通用 helper 需要自己保证支持 --output 或等价输出目录参数。",
            "如果 doctor 预览参数和 helper 实际契约不一致，先改 helper 配置而不是改主进程。",
        ],
    }
}

fn helper_binary_status(helper_bin: &str) -> HelperBinaryStatus {
    let resolved_path = resolve_helper_bin_path(helper_bin);
    let exists = resolved_path.as_ref().is_some_and(|path| path.is_file());
    let executable = resolved_path
        .as_ref()
        .is_some_and(|path| is_executable_file(path));
    HelperBinaryStatus {
        resolved_path,
        exists,
        executable,
    }
}

fn resolve_helper_bin_path(helper_bin: &str) -> Option<PathBuf> {
    let trimmed = helper_bin.trim();
    if trimmed.is_empty() {
        return None;
    }

    let candidate = PathBuf::from(trimmed);
    if candidate.components().count() > 1 || trimmed.contains(std::path::MAIN_SEPARATOR) {
        return Some(candidate);
    }

    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(trimmed))
            .find(|path| path.is_file())
    })
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn wechat_article_url_doctor_next_steps(
    context: WechatArticleDoctorContext<'_>,
) -> Vec<&'static str> {
    let mut steps = Vec::new();
    if context.helper_bin.trim().is_empty() {
        steps.push("configure_public_sources_wechat_article_helper_bin");
    }
    if !context.helper_bin.trim().is_empty() && !context.helper_status.exists {
        steps.push("install_or_fix_helper_bin_path");
    }
    if context.helper_status.exists && !context.helper_status.executable {
        steps.push("mark_helper_bin_executable");
    }
    if !context.output_dir_declared {
        steps.push("configure_public_sources_wechat_article_helper_output_dir");
    } else if !context.output_dir_exists && !context.output_dir_parent_exists {
        steps.push("create_output_dir_parent_before_running_helper");
    }
    if context.requested_url.is_some() && context.normalized_url.is_none() {
        steps.push("use_full_mp_weixin_article_url");
    }
    match context.helper_kind {
        WechatArticleHelperKind::Noisepoint => {
            steps.push("expect_flat_markdown_output_in_run_dir");
        }
        WechatArticleHelperKind::Jackwener => {
            steps.push("expect_nested_article_directory_under_run_dir");
        }
        WechatArticleHelperKind::Generic => {
            steps.push("verify_generic_helper_supports_output_dir_argument");
        }
    }
    steps.push("doctor_is_read_only_then_run_wechat_article_url_when_ready");
    steps
}

fn recommended_doctor_command(url: Option<&str>, output_dir: Option<&Path>) -> String {
    let mut command = String::from("cargo run -- wechat-article-url-doctor");
    if let Some(url) = url {
        command.push_str(&format!(" --url '{}'", url));
    }
    if let Some(output_dir) = output_dir {
        command.push_str(&format!(" --output-dir '{}'", output_dir.display()));
    }
    command
}

fn non_empty_excerpt(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(truncate_chars(trimmed, 400))
    }
}

fn current_unix_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

fn discover_helper_output(root: &Path) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    if let Some(markdown_path) = first_markdown_in_dir(root) {
        let article_dir = Some(root.to_path_buf());
        let images_dir = root.join("images");
        return (
            article_dir,
            Some(markdown_path),
            images_dir.is_dir().then_some(images_dir),
        );
    }

    let markdown_path = collect_markdown_files(root, 3).into_iter().next();
    let article_dir = markdown_path
        .as_ref()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    let images_dir = article_dir
        .as_ref()
        .map(|dir| dir.join("images"))
        .filter(|path| path.is_dir());
    (article_dir, markdown_path, images_dir)
}

fn collect_markdown_files(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_markdown_files_inner(root, max_depth, &mut files);
    files.sort();
    files
}

fn collect_markdown_files_inner(root: &Path, depth_left: usize, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };

    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            files.push(path);
            continue;
        }

        if depth_left > 0 && path.is_dir() {
            collect_markdown_files_inner(&path, depth_left - 1, files);
        }
    }
}

fn first_markdown_in_dir(dir: &Path) -> Option<PathBuf> {
    let mut files = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
        .collect::<Vec<_>>();
    files.sort();
    files.into_iter().next()
}

fn parse_wechat_article_markdown(path: &Path) -> Result<WechatArticleMarkdown> {
    let content = std::fs::read_to_string(path)?;
    parse_wechat_article_markdown_str(&content)
}

fn parse_wechat_article_markdown_str(content: &str) -> Result<WechatArticleMarkdown> {
    let mut title = String::new();
    let mut account_name = None;
    let mut published_at = None;
    let mut source_url = None;
    let mut in_header = true;
    let mut body_lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if title.is_empty()
            && let Some(rest) = trimmed.strip_prefix("# ")
        {
            title = rest.trim().to_string();
            continue;
        }

        if in_header {
            if let Some(rest) = trimmed.strip_prefix("> 公众号:") {
                let value = rest.trim();
                if !value.is_empty() {
                    account_name = Some(value.to_string());
                }
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("> 发布时间:") {
                let value = rest.trim();
                if !value.is_empty() {
                    published_at = Some(value.to_string());
                }
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("> 原文链接:") {
                let value = rest.trim();
                if !value.is_empty() {
                    source_url = Some(value.to_string());
                }
                continue;
            }
            if trimmed == "---" {
                in_header = false;
                continue;
            }
            if trimmed.is_empty() {
                continue;
            }
            in_header = false;
        }

        if !trimmed.is_empty() {
            body_lines.push(trimmed.to_string());
        }
    }

    if title.is_empty() {
        return Err(QunMindError::Other(anyhow::anyhow!(
            "helper 生成的 markdown 缺少标题"
        )));
    }

    Ok(WechatArticleMarkdown {
        title,
        account_name,
        published_at,
        source_url,
        summary: extract_summary_from_body(&body_lines),
    })
}

fn extract_summary_from_body(lines: &[String]) -> Option<String> {
    let summary = lines
        .iter()
        .filter(|line| !line.starts_with("!["))
        .filter(|line| !line.starts_with("```"))
        .filter(|line| !line.starts_with('>'))
        .filter(|line| !line.starts_with('#'))
        .filter(|line| !line.starts_with("- "))
        .find(|line| !line.is_empty())?
        .trim();

    if summary.is_empty() {
        None
    } else {
        Some(truncate_chars(summary, 180))
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_wechat_article_url() {
        let err = normalize_wechat_article_url("https://example.com/test").unwrap_err();
        assert!(err.to_string().contains("mp.weixin.qq.com"));
    }

    #[test]
    fn accepts_wechat_article_url() {
        let url = normalize_wechat_article_url("https://mp.weixin.qq.com/s/abc").unwrap();
        assert_eq!(url, "https://mp.weixin.qq.com/s/abc");
    }

    #[test]
    fn parses_helper_markdown_metadata_and_summary() {
        let markdown = r#"# 一篇公众号文章

> 公众号: 寻月隐君
> 发布时间: 2026-07-01 08:00:00
> 原文链接: https://mp.weixin.qq.com/s/example

---

这是正文第一段，用来测试摘要提取。

![](images/img_001.png)
"#;

        let parsed = parse_wechat_article_markdown_str(markdown).unwrap();

        assert_eq!(parsed.title, "一篇公众号文章");
        assert_eq!(parsed.account_name.as_deref(), Some("寻月隐君"));
        assert_eq!(parsed.published_at.as_deref(), Some("2026-07-01 08:00:00"));
        assert_eq!(
            parsed.source_url.as_deref(),
            Some("https://mp.weixin.qq.com/s/example")
        );
        assert_eq!(
            parsed.summary.as_deref(),
            Some("这是正文第一段，用来测试摘要提取。")
        );
    }

    #[test]
    fn parses_markdown_without_metadata_separator() {
        let markdown = r#"# 一篇公众号文章

这是没有头部分隔线时的正文第一段。

第二段补充说明。
"#;

        let parsed = parse_wechat_article_markdown_str(markdown).unwrap();

        assert_eq!(parsed.title, "一篇公众号文章");
        assert_eq!(
            parsed.summary.as_deref(),
            Some("这是没有头部分隔线时的正文第一段。")
        );
    }

    #[test]
    fn errors_when_helper_markdown_has_no_title() {
        let err = parse_wechat_article_markdown_str("> 公众号: 寻月隐君").unwrap_err();
        assert!(err.to_string().contains("缺少标题"));
    }

    #[test]
    fn detects_known_helper_kinds_from_binary_name() {
        assert_eq!(
            detect_helper_kind("/usr/local/bin/mp-weixin-to-md"),
            WechatArticleHelperKind::Noisepoint
        );
        assert_eq!(
            detect_helper_kind("/Users/test/.local/bin/wechat-article-to-markdown"),
            WechatArticleHelperKind::Jackwener
        );
        assert_eq!(
            detect_helper_kind("/opt/tools/custom-helper"),
            WechatArticleHelperKind::Generic
        );
    }

    #[test]
    fn helper_args_use_flat_output_for_noisepoint() {
        let args = helper_args_for(
            WechatArticleHelperKind::Noisepoint,
            "https://mp.weixin.qq.com/s/example",
            Path::new("/tmp/wechat-helper/run-1"),
        );

        assert_eq!(
            args,
            vec![
                "https://mp.weixin.qq.com/s/example".to_string(),
                "-o".to_string(),
                "article.md".to_string(),
                "--download-assets".to_string(),
                "--assets-dir".to_string(),
                "images".to_string()
            ]
        );
    }

    #[test]
    fn helper_args_use_url_only_for_jackwener() {
        let args = helper_args_for(
            WechatArticleHelperKind::Jackwener,
            "https://mp.weixin.qq.com/s/example",
            Path::new("/tmp/wechat-helper/run-2"),
        );

        assert_eq!(args, vec!["https://mp.weixin.qq.com/s/example".to_string()]);
    }

    #[test]
    fn helper_args_use_output_dir_for_generic_helper() {
        let args = helper_args_for(
            WechatArticleHelperKind::Generic,
            "https://mp.weixin.qq.com/s/example",
            Path::new("/tmp/wechat-helper/run-3"),
        );

        assert_eq!(
            args,
            vec![
                "https://mp.weixin.qq.com/s/example".to_string(),
                "--output".to_string(),
                "/tmp/wechat-helper/run-3".to_string()
            ]
        );
    }

    #[test]
    fn discover_helper_output_supports_flat_markdown_layout() {
        let root = PathBuf::from(format!(
            "/tmp/qunmind-wechat-helper-flat-{}",
            current_unix_timestamp_millis()
        ));
        std::fs::create_dir_all(root.join("images")).unwrap();
        std::fs::write(root.join("article.md"), "# 标题\n\n正文").unwrap();

        let (article_dir, markdown_path, images_dir) = discover_helper_output(&root);
        let markdown_file = root.join("article.md");
        let images_path = root.join("images");

        assert_eq!(article_dir.as_deref(), Some(root.as_path()));
        assert_eq!(markdown_path.as_deref(), Some(markdown_file.as_path()));
        assert_eq!(images_dir.as_deref(), Some(images_path.as_path()));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discover_helper_output_supports_nested_article_directory() {
        let root = PathBuf::from(format!(
            "/tmp/qunmind-wechat-helper-nested-{}",
            current_unix_timestamp_millis()
        ));
        let article_dir_path = root.join("output").join("some-article");
        std::fs::create_dir_all(article_dir_path.join("images")).unwrap();
        std::fs::write(article_dir_path.join("some-article.md"), "# 标题\n\n正文").unwrap();

        let (article_dir, markdown_path, images_dir) = discover_helper_output(&root);
        let markdown_file = article_dir_path.join("some-article.md");
        let images_path = article_dir_path.join("images");

        assert_eq!(article_dir.as_deref(), Some(article_dir_path.as_path()));
        assert_eq!(markdown_path.as_deref(), Some(markdown_file.as_path()));
        assert_eq!(images_dir.as_deref(), Some(images_path.as_path()));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn response_json_includes_nested_parsed_object() {
        let output = WechatArticleUrlOutput {
            url: "https://mp.weixin.qq.com/s/example".to_string(),
            helper_bin: "wechat-article-to-markdown".to_string(),
            helper_kind: "jackwener".to_string(),
            output_dir: PathBuf::from("/tmp/qunmind-wechat-helper"),
            run_dir: PathBuf::from("/tmp/qunmind-wechat-helper/run-1"),
            article_dir: Some(PathBuf::from("/tmp/qunmind-wechat-helper/article")),
            markdown_path: Some(PathBuf::from(
                "/tmp/qunmind-wechat-helper/article/article.md",
            )),
            images_dir: Some(PathBuf::from("/tmp/qunmind-wechat-helper/article/images")),
            parsed: Some(WechatArticleMarkdown {
                title: "一篇公众号文章".to_string(),
                account_name: Some("寻月隐君".to_string()),
                published_at: Some("2026-07-01 08:00:00".to_string()),
                source_url: Some("https://mp.weixin.qq.com/s/example".to_string()),
                summary: Some("这是正文第一段，用来测试摘要提取。".to_string()),
            }),
        };

        let json = wechat_article_url_response_json(&output);

        assert_eq!(json["title"], "一篇公众号文章");
        assert_eq!(json["helper_kind"], "jackwener");
        assert_eq!(json["parsed"]["title"], "一篇公众号文章");
        assert_eq!(json["parsed"]["account_name"], "寻月隐君");
        assert_eq!(json["parsed"]["published_at"], "2026-07-01 08:00:00");
        assert_eq!(
            json["parsed"]["source_url"],
            "https://mp.weixin.qq.com/s/example"
        );
        assert_eq!(
            json["parsed"]["summary"],
            "这是正文第一段，用来测试摘要提取。"
        );
    }

    #[test]
    fn failure_json_includes_doctor_command_and_stage() {
        let failure = WechatArticleUrlFailure {
            requested_url: Some("https://mp.weixin.qq.com/s/example".to_string()),
            normalized_url: Some("https://mp.weixin.qq.com/s/example".to_string()),
            helper_bin: "/usr/local/bin/mp-weixin-to-md".to_string(),
            helper_kind: "noisepoint".to_string(),
            output_dir: Some(PathBuf::from("/tmp/qunmind-wechat-helper")),
            run_dir: Some(PathBuf::from("/tmp/qunmind-wechat-helper/run-1")),
            failure_stage: "helper_exit_non_zero",
            message: "公众号单链接 helper 执行失败: bad gateway".to_string(),
            stdout_excerpt: None,
            stderr_excerpt: Some("bad gateway".to_string()),
        };

        let json = wechat_article_url_failure_json(&failure);

        assert_eq!(json["ok"], false);
        assert_eq!(json["failure_stage"], "helper_exit_non_zero");
        assert_eq!(json["helper_kind"], "noisepoint");
        assert_eq!(
            json["recommended_doctor_command"],
            "cargo run -- wechat-article-url-doctor --url 'https://mp.weixin.qq.com/s/example' --output-dir '/tmp/qunmind-wechat-helper'"
        );
        assert_eq!(json["doctor"]["doctor"], true);
    }

    #[test]
    fn doctor_reports_missing_helper_config_as_blocker() {
        let config = PublicSourcesConfig::default();

        let json = wechat_article_url_doctor_json(&config, None, None);

        assert_eq!(json["doctor"], true);
        assert_eq!(json["ok"], false);
        assert!(
            json["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "helper_bin_missing")
        );
    }

    #[test]
    fn doctor_reports_known_helper_kind_and_run_dir_example() {
        let config = PublicSourcesConfig {
            wechat_article_helper_bin: "/usr/local/bin/mp-weixin-to-md".to_string(),
            wechat_article_helper_output_dir: "/tmp/qunmind-wechat-helper".to_string(),
            ..PublicSourcesConfig::default()
        };

        let json = wechat_article_url_doctor_json(
            &config,
            Some("https://mp.weixin.qq.com/s/example"),
            None,
        );

        assert_eq!(json["helper"]["kind"], "noisepoint");
        assert!(
            json["helper"]["known_advice"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value.as_str().unwrap().contains("run_dir/images"))
        );
        assert_eq!(json["url"], "https://mp.weixin.qq.com/s/example");
        assert!(
            json["output"]["run_dir_example"]
                .as_str()
                .unwrap()
                .contains("/tmp/qunmind-wechat-helper/run-")
        );
    }
}
