// WeChat local database reader — pure Rust.
//
// Architecture:
//   mach2 (Rust)     →  scan WeChat process memory for SQLCipher raw key
//   lldb (subprocess) →  fallback: hook CCKeyDerivationPBKDF to capture key material
//   pbkdf2 (Rust)     →  derive final AES-256 key from captured material
//   aes + cbc (Rust)  →  decrypt SQLCipher pages in pure Rust, no system libs
//   rusqlite/bundled  →  open the decrypted temp DB and query messages
use std::fs;
use std::path::PathBuf;

use aes::Aes256;
use aes::cipher::{BlockDecryptMut, KeyIvInit};
use tracing::{debug, info, warn};

use crate::error::{QunMindError, Result};

const PAGE_SIZE: usize = 4096;
const RESERVE_SIZE: usize = 80; // IV(16) + HMAC-SHA512(64), per SQLCipher 4
const SALT_SIZE: usize = 16;

fn aes_cbc_decrypt(key: &[u8; 32], iv: &[u8; 16], data: &[u8]) -> Result<Vec<u8>> {
    use aes::cipher::Block;
    if data.is_empty() || !data.len().is_multiple_of(16) {
        return Err(QunMindError::Channel(format!(
            "密文长度不是 AES 块大小的倍数: {}",
            data.len()
        )));
    }
    let mut blocks: Vec<Block<Aes256>> = data
        .chunks_exact(16)
        .map(Block::<Aes256>::clone_from_slice)
        .collect();
    cbc::Decryptor::<Aes256>::new(key.into(), iv.into()).decrypt_blocks_mut(&mut blocks);
    Ok(blocks.iter().flat_map(|b| b.iter().copied()).collect())
}

// ---------------------------------------------------------------------------
// WeChat data paths
// ---------------------------------------------------------------------------

const WECHAT_CONTAINERS: &[&str] = &["com.tencent.xinWeChat", "com.tencent.xinWeChat.beta"];

fn wechat_data_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok().map(PathBuf::from)?;
    let containers = home.join("Library/Containers");

    for container in WECHAT_CONTAINERS {
        let base = containers
            .join(container)
            .join("Data/Documents/xwechat_files");
        if !base.exists() {
            continue;
        }
        let entries = match std::fs::read_dir(&base) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && entry.file_name().to_string_lossy().starts_with("wxid_")
            {
                let db = entry.path().join("db_storage/message");
                if db.exists() {
                    return Some(db);
                }
            }
        }
    }

    None
}

pub fn message_db_paths() -> Vec<PathBuf> {
    let dir = match wechat_data_dir() {
        Some(d) => d,
        None => return Vec::new(),
    };

    message_db_paths_in_dir(&dir)
}

fn message_db_paths_in_dir(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                // Only the sharded message databases: message_0.db, message_1.db, …
                // Exclude WAL/SHM files, kvdb, fts, resource, and .material files.
                name.starts_with("message_")
                    && name.ends_with(".db")
                    && !name.ends_with("_fts.db")
                    && !name.ends_with("_resource.db")
                    && name
                        .strip_prefix("message_")
                        .and_then(|s| s.strip_suffix(".db"))
                        .map(|s| s.parse::<u32>().is_ok())
                        .unwrap_or(false)
            })
            .map(|e| e.path())
            .collect(),
        Err(_) => return Vec::new(),
    };

    paths.sort_by(|a, b| {
        let n = |p: &PathBuf| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_prefix("message_"))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0)
        };
        n(b).cmp(&n(a))
    });

    paths
}

// ---------------------------------------------------------------------------
// Memory scan for SQLCipher key (macOS only)
// ---------------------------------------------------------------------------

/// Scan all WeChat-related processes and return every distinct raw key found.
/// Each entry is a 96-char hex string: first 64 = AES-256 key, last 32 = db salt.
/// The salt matches the first 16 bytes of the encrypted database file.
#[cfg(target_os = "macos")]
pub fn extract_all_db_keys() -> Result<Vec<String>> {
    use mach2::kern_return::KERN_SUCCESS;
    use mach2::port::mach_port_t;
    use mach2::traps::{mach_task_self, task_for_pid};
    use mach2::vm::mach_vm_region;
    use mach2::vm_prot::{VM_PROT_READ, VM_PROT_WRITE};
    use mach2::vm_region::{VM_REGION_BASIC_INFO_64, vm_region_basic_info_64};
    use mach2::vm_types::{mach_vm_address_t, mach_vm_size_t};

    unsafe extern "C" {
        fn mach_vm_read(
            task: mach_port_t,
            addr: mach_vm_address_t,
            size: mach_vm_size_t,
            data: *mut usize,
            data_cnt: *mut u32,
        ) -> i32;
        fn mach_vm_deallocate(
            task: mach_port_t,
            addr: mach_vm_address_t,
            size: mach_vm_size_t,
        ) -> i32;
    }

    const CHUNK_SIZE: u64 = 2 * 1024 * 1024;
    const OVERLAP: u64 = 99;

    let pids = find_wechat_pids()?;
    debug!(?pids, "扫描微信进程");
    let mut all_keys: Vec<String> = Vec::new();

    for pid in pids {
        let mut task: mach_port_t = 0;
        let kr = unsafe { task_for_pid(mach_task_self(), pid, &mut task) };
        if kr != KERN_SUCCESS {
            debug!(pid, kr, "task_for_pid 失败, 跳过");
            continue;
        }
        debug!(pid, "task_for_pid 成功, 开始扫描内存");

        let mut address: mach_vm_address_t = 0;
        loop {
            let mut size: mach_vm_size_t = 0;
            let mut info: vm_region_basic_info_64 = unsafe { std::mem::zeroed() };
            let mut info_count = vm_region_basic_info_64::count();
            let mut object_name: mach_port_t = 0;

            let kr = unsafe {
                mach_vm_region(
                    task,
                    &mut address,
                    &mut size,
                    VM_REGION_BASIC_INFO_64,
                    (&mut info as *mut _) as mach2::vm_region::vm_region_info_t,
                    &mut info_count,
                    &mut object_name,
                )
            };
            if kr != KERN_SUCCESS {
                break;
            }
            if size == 0 {
                address = address.saturating_add(1);
                continue;
            }

            let rw = VM_PROT_READ | VM_PROT_WRITE;
            if (info.protection & rw) == rw {
                let end = address + size;
                let mut ca = address;
                while ca < end {
                    let cs = (end - ca).min(CHUNK_SIZE);
                    let mut data_ptr: usize = 0;
                    let mut data_cnt: u32 = 0;
                    let kr = unsafe { mach_vm_read(task, ca, cs, &mut data_ptr, &mut data_cnt) };
                    if kr == KERN_SUCCESS && data_cnt > 0 {
                        let buf = unsafe {
                            std::slice::from_raw_parts(data_ptr as *const u8, data_cnt as usize)
                        };
                        search_key_pattern(buf, &mut all_keys);
                        unsafe {
                            mach_vm_deallocate(mach_task_self(), data_ptr as u64, data_cnt as u64);
                        }
                    }
                    ca += if cs > OVERLAP { cs - OVERLAP } else { cs };
                }
            }

            address = address.saturating_add(size);
        }
        debug!(pid, keys_found = all_keys.len(), "进程扫描完成");
    }

    if all_keys.is_empty() {
        return Err(QunMindError::Channel(
            "无法从微信进程内存中提取数据库密钥。\
            请确认微信已登录，并以 sudo 运行。\
            参考：https://github.com/Thearas/wechat-db-decrypt-macos"
                .to_string(),
        ));
    }

    Ok(all_keys)
}

/// Find the key for a specific db.
#[cfg(target_os = "macos")]
pub fn extract_db_key() -> Result<String> {
    extract_all_db_keys()?
        .into_iter()
        .next()
        .ok_or_else(|| QunMindError::Channel("未找到任何数据库密钥".to_string()))
}

#[cfg(not(target_os = "macos"))]
pub fn extract_all_db_keys() -> Result<Vec<String>> {
    Err(QunMindError::Channel(
        "当前平台暂不支持微信内存扫描".to_string(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn extract_db_key() -> Result<String> {
    Err(QunMindError::Channel(
        "当前平台暂不支持微信内存扫描".to_string(),
    ))
}

/// Return PIDs for all WeChat-related processes (main + sub-processes like WeChatAppEx).
/// WeChat 4.x is multi-process: the database is typically opened inside WeChatAppEx.
#[cfg(target_os = "macos")]
fn find_wechat_pids() -> Result<Vec<i32>> {
    use std::process::Command;

    // Match any process whose name starts with "WeChat"
    let output = Command::new("pgrep")
        .args(["-f", "WeChat"])
        .output()
        .map_err(|_| QunMindError::Channel("无法查找微信进程".to_string()))?;

    let pids: Vec<i32> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|s| s.trim().parse::<i32>().ok())
        .collect();

    if pids.is_empty() {
        return Err(QunMindError::Channel(
            "微信未运行，请先启动微信".to_string(),
        ));
    }

    Ok(pids)
}

/// Scan `buf` for SQLCipher raw key patterns: `x'<hex>'`
///
/// Supported hex lengths:
///   - 64: key only (32 bytes)
///   - 96: key (64) + salt (32)
///   - 98..192 even: longer patterns, first 64 = key, last 32 = salt
///
/// Keys are deduplicated. Accepts both upper and lower case hex.
#[cfg(target_os = "macos")]
fn search_key_pattern(buf: &[u8], keys: &mut Vec<String>) {
    if buf.len() < 67 {
        return;
    }
    let mut i = 0;
    while i + 67 <= buf.len() {
        if buf[i] != b'x' || buf[i + 1] != b'\'' {
            i += 1;
            continue;
        }
        let hex_start = i + 2;

        // Find end of hex run (up to reasonable max)
        let mut hex_end = hex_start;
        while hex_end < buf.len() && hex_end - hex_start < 200 && buf[hex_end].is_ascii_hexdigit() {
            hex_end += 1;
        }
        if hex_end >= buf.len() || buf[hex_end] != b'\'' {
            i += 1;
            continue;
        }

        let hex_len = hex_end - hex_start;
        // Only accept supported lengths
        if hex_len != 64 && hex_len != 96 && !(hex_len > 96 && hex_len <= 192 && hex_len % 2 == 0) {
            i += 1;
            continue;
        }

        // Extract key (first 64 hex) and optionally salt (last 32 hex)
        let key_part =
            String::from_utf8_lossy(&buf[hex_start..hex_start + 64]).to_ascii_lowercase();

        if hex_len >= 96 {
            let _salt_start = hex_end - 32;
            let full =
                String::from_utf8_lossy(&buf[hex_start..hex_start + 96]).to_ascii_lowercase();
            if !keys.iter().any(|k| k == &full) {
                debug!(
                    key_prefix = &key_part[..16],
                    key_salt = &full[64..96],
                    "发现密钥(96)"
                );
                keys.push(full);
            }
        } else {
            // 64-char key only
            if !keys.iter().any(|k| k == &key_part) {
                debug!(key_prefix = &key_part[..16], "发现密钥(64)");
                keys.push(key_part);
            }
        }

        i = hex_end + 1;
    }

    // Also search for codec_ctx structures by finding raw salt bytes
    search_codec_ctx(buf, keys);
}

/// Search memory for SQLCipher codec_ctx by finding known DB salt bytes.
/// The derived AES key is typically near the salt in the struct.
/// Tries multiple offsets around the salt position.
#[cfg(target_os = "macos")]
fn search_codec_ctx(buf: &[u8], keys: &mut Vec<String>) {
    let targets: &[(&[u8; 16], &str)] = &[
        (
            &[
                0xc0, 0x10, 0xb8, 0x02, 0xac, 0x01, 0x1a, 0x2a, 0x07, 0xeb, 0x16, 0xee, 0x59, 0x4f,
                0x8b, 0x2c,
            ],
            "msg0",
        ),
        (
            &[
                0xb9, 0x29, 0xc9, 0xfb, 0x7d, 0x15, 0x9e, 0x90, 0x68, 0x28, 0x58, 0xe7, 0xf0, 0x99,
                0x1c, 0x79,
            ],
            "msg1",
        ),
    ];

    for (needle, label) in targets {
        if buf.len() < 80 {
            continue;
        }
        let first = needle[0];
        let mut pos = 0;
        while pos + 80 <= buf.len() {
            if buf[pos] != first {
                pos += 1;
                continue;
            }
            if &buf[pos..pos + 16] != needle.as_slice() {
                pos += 1;
                continue;
            }

            // Salt at pos. Try key at offsets: -48, -32, -16, 0, +16, +32, +48
            let key_offsets: &[isize] = &[-48, -32, -16, 0, 16, 32, 48];
            for &off in key_offsets {
                let key_pos = match off {
                    o if o >= 0 => pos + o as usize,
                    o => match pos.checked_sub((-o) as usize) {
                        Some(p) => p,
                        None => continue,
                    },
                };
                if key_pos + 32 > buf.len() {
                    continue;
                }
                let ck: &[u8; 32] = buf[key_pos..key_pos + 32].try_into().unwrap();

                // Skip salt itself (off=0 gives salt, not a key)
                if off == 0 {
                    continue;
                }
                // Skip if looks like ASCII text (real AES keys have <25% printable ASCII)
                let ascii = ck
                    .iter()
                    .filter(|&&b| b.is_ascii_graphic() || b == 0x20 || b == 0x00)
                    .count();
                if ascii > 16 {
                    continue;
                }
                // Skip if more than half zero bytes
                if ck.iter().filter(|&&b| b == 0).count() > 20 {
                    continue;
                }

                let kh: String = ck.iter().map(|b| format!("{b:02x}")).collect();
                let sh: String = needle.iter().map(|b| format!("{b:02x}")).collect();
                let full = format!("{kh}{sh}");
                if !keys.iter().any(|k| k == &full) {
                    debug!(label, off, key_prefix = &kh[..16], "codec_ctx候选");
                    keys.push(full);
                }
            }
            pos += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// LLDB-based key extraction — hooks CCKeyDerivationPBKDF at WeChat startup
// Then derives final AES key via macOS built-in `openssl kdf PBKDF2`.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub fn lldb_extract_keys() -> Result<Vec<String>> {
    extract_keys_via_lldb()
}

#[cfg(not(target_os = "macos"))]
pub fn lldb_extract_keys() -> Result<Vec<String>> {
    Err(QunMindError::Channel(
        "LLDB 密钥提取仅支持 macOS".to_string(),
    ))
}

#[cfg(target_os = "macos")]
fn extract_keys_via_lldb() -> Result<Vec<String>> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let db_paths = message_db_paths();
    if db_paths.is_empty() {
        return Err(QunMindError::Channel("未找到微信消息数据库".to_string()));
    }
    let mut db_salts: Vec<([u8; 16], PathBuf)> = Vec::new();
    for p in &db_paths {
        let mut f = match fs::File::open(p) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut s = [0u8; 16];
        use std::io::Read;
        if f.read_exact(&mut s).is_ok() {
            db_salts.push((s, p.clone()));
        }
    }

    let tmp = std::env::temp_dir().join("qunmind_lldb");
    let _ = fs::create_dir_all(&tmp);
    let py = tmp.join("hook.py");
    fs::write(&py, LLDB_HOOK_SCRIPT)
        .map_err(|e| QunMindError::Channel(format!("lldb脚本: {e}")))?;

    // Force kill all WeChat processes
    let _ = Command::new("pkill")
        .arg("-9")
        .arg("-f")
        .arg("WeChat")
        .output();
    for _ in 0..10 {
        if Command::new("pgrep")
            .arg("-x")
            .arg("WeChat")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(true)
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = Command::new("pkill")
        .arg("-9")
        .arg("-f")
        .arg("WeChatAppEx")
        .output();
    std::thread::sleep(std::time::Duration::from_millis(500));

    info!("微信已关闭，等待重启。请在手机上确认登录...");

    let mut lldb = Command::new("lldb")
        .args([
            "-w",
            "-n",
            "WeChat",
            "-o",
            &format!("command script import {}", py.display()),
            "-o",
            "go",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| QunMindError::Channel(format!("lldb: {e}")))?;

    std::thread::sleep(std::time::Duration::from_millis(500));
    let _ = Command::new("open").arg("-a").arg("WeChat").output();

    let mut results: Vec<String> = Vec::new();
    for line in BufReader::new(lldb.stdout.take().unwrap()).lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if !line.starts_with("[QUNMIND_KEY]") {
            continue;
        }
        let pwd = extract_field(&line, "pwd=");
        let salt_hex = extract_field(&line, "salt=");
        debug!(pwd_len = pwd.len(), salt_hex, "LLDB 捕获到 PBKDF2 密钥");
        for (db_salt, db_path) in &db_salts {
            let ds_hex: String = db_salt.iter().map(|b| format!("{b:02x}")).collect();
            if salt_hex != ds_hex || results.iter().any(|k| k.ends_with(&ds_hex)) {
                continue;
            }
            let key = match pbkdf2_derive_openssl(&pwd, &ds_hex) {
                Some(k) => k,
                None => continue,
            };
            let key_hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
            if let Ok(enc) = fs::read(db_path)
                && enc.len() >= PAGE_SIZE
            {
                let iv: &[u8; 16] = enc[PAGE_SIZE - RESERVE_SIZE..PAGE_SIZE - RESERVE_SIZE + 16]
                    .try_into()
                    .unwrap();
                if let Ok(dec) =
                    aes_cbc_decrypt(&key, iv, &enc[SALT_SIZE..PAGE_SIZE - RESERVE_SIZE])
                    && dec.len() >= 16
                    && &dec[..15] == b"SQLite format 3"
                {
                    info!(path=%db_path.display(), "LLDB+PBKDF2 解密成功");
                    results.push(format!("{key_hex}{ds_hex}"));
                }
            }
        }
        if results.len() >= db_salts.len() {
            break;
        }
    }
    let _ = lldb.kill();
    let _ = fs::remove_file(&py);
    if results.is_empty() {
        Err(QunMindError::Channel("LLDB未捕获到PBKDF2密钥".to_string()))
    } else {
        Ok(results)
    }
}

#[cfg(target_os = "macos")]
fn extract_field(line: &str, prefix: &str) -> String {
    line.split_whitespace()
        .find(|s| s.starts_with(prefix))
        .map(|s| s[prefix.len()..].to_string())
        .unwrap_or_default()
}

/// PBKDF2-HMAC-SHA512 via macOS built-in openssl (zero extra deps).
#[cfg(target_os = "macos")]
fn pbkdf2_derive_openssl(password_hex: &str, salt_hex: &str) -> Option<[u8; 32]> {
    use std::process::Command;
    let out = Command::new("openssl")
        .args([
            "kdf",
            "-keylen",
            "32",
            "-kdfopt",
            "digest:SHA512",
            "-kdfopt",
            "iter:256000",
            "-kdfopt",
            &format!("hexsalt:{salt_hex}"),
            "-kdfopt",
            &format!("hexpass:{password_hex}"),
            "PBKDF2",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let hex = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let bytes = hex_decode(&hex).ok()?;
    if bytes.len() < 32 {
        return None;
    }
    let mut k = [0u8; 32];
    k.copy_from_slice(&bytes[..32]);
    Some(k)
}

#[cfg(target_os = "macos")]
const LLDB_HOOK_SCRIPT: &str = r#"import lldb,binascii
def h(f,b,d):
 g=f.GetRegisters()[0]
 p1=g.GetChildMemberWithName("x1").GetValueAsUnsigned()
 l2=g.GetChildMemberWithName("x2").GetValueAsUnsigned()
 p3=g.GetChildMemberWithName("x3").GetValueAsUnsigned()
 l4=g.GetChildMemberWithName("x4").GetValueAsUnsigned()
 r6=g.GetChildMemberWithName("x6").GetValueAsUnsigned()
 if r6!=256000 or l2==0 or l4==0 or l2>1024 or l4>1024:return False
 e=lldb.SBError();p=f.GetThread().GetProcess()
 pw=p.ReadMemory(p1,l2,e);ps=p.ReadMemory(p3,l4,e)
 print(f"[QUNMIND_KEY] pwd={binascii.hexlify(pw).decode()} salt={binascii.hexlify(ps).decode()}",flush=True)
 return False
def s(d,c,r,u):
 t=d.GetSelectedTarget()
 b=t.BreakpointCreateByName("CCKeyDerivationPBKDF")
 b.SetScriptCallbackFunction(__name__+".h");b.SetAutoContinue(True)
 t.GetProcess().Continue()
def __lldb_init_module(d,u):d.HandleCommand("command script add -f %s.s go"%__name__)
"#;

// ---------------------------------------------------------------------------
// Pure Rust SQLCipher 4 page decryption
// ---------------------------------------------------------------------------

/// Decrypt a SQLCipher-encrypted database file in pure Rust.
///
/// Returns the path to a temporary plaintext SQLite database.
///
/// Key matching strategy:
///   1. Salt-based: for 96-char keys, match by embedded salt (last 32 hex chars).
///   2. Brute-force: try every 64/96-char key against page 1, verify SQLite header.
pub fn decrypt_db(encrypted_path: &std::path::Path, all_keys: &[String]) -> Result<PathBuf> {
    let file_salt = {
        let mut f = fs::File::open(encrypted_path)
            .map_err(|e| QunMindError::Channel(format!("无法读取加密数据库: {e}")))?;
        let mut salt = [0u8; SALT_SIZE];
        use std::io::Read;
        f.read_exact(&mut salt)
            .map_err(|e| QunMindError::Channel(format!("无法读取数据库 salt: {e}")))?;
        salt.iter().map(|b| format!("{b:02x}")).collect::<String>()
    };

    let encrypted = fs::read(encrypted_path)
        .map_err(|e| QunMindError::Channel(format!("无法读取加密数据库: {e}")))?;
    if encrypted.len() < PAGE_SIZE {
        return Err(QunMindError::Channel(format!(
            "数据库文件过小: {}",
            encrypted_path.display()
        )));
    }

    // Strategy 1: salt-based match for 96-char keys
    if let Some(k) = all_keys
        .iter()
        .find(|k| k.len() == 96 && k[64..] == file_salt)
    {
        return decrypt_with_key(encrypted_path, k, &encrypted);
    }

    // Strategy 2: brute-force try every key against page 1
    let total = all_keys.len();
    for (i, k) in all_keys.iter().enumerate() {
        if k.len() != 64 && k.len() != 96 {
            continue;
        }
        debug!(
            "[{}/{}] 尝试密钥 {}...",
            i + 1,
            total,
            &k[..16.min(k.len())]
        );
        if try_key_on_page1(k, &encrypted) {
            info!(
                "[{}/{}] 密钥 {} 解密成功!",
                i + 1,
                total,
                &k[..16.min(k.len())]
            );
            return decrypt_with_key(encrypted_path, k, &encrypted);
        }
    }

    let cand_info: Vec<String> = all_keys
        .iter()
        .map(|k| format!("{}..", &k[..16.min(k.len())]))
        .collect();
    Err(QunMindError::Channel(format!(
        "内存中未找到匹配数据库 {} 的密钥 (salt={}, candidates=[{}])",
        encrypted_path.display(),
        file_salt,
        cand_info.join(", ")
    )))
}

fn try_key_on_page1(key_hex: &str, encrypted: &[u8]) -> bool {
    // Direct try: use key bytes as-is
    let key_bytes = match hex_decode(&key_hex[..64.min(key_hex.len())]) {
        Ok(b) => b,
        Err(_) => return false,
    };
    if key_bytes.len() >= 32 {
        let aes_key: &[u8; 32] = key_bytes[..32].try_into().unwrap();
        let page = &encrypted[..PAGE_SIZE];
        let iv: &[u8; 16] = page[PAGE_SIZE - RESERVE_SIZE..PAGE_SIZE - RESERVE_SIZE + 16]
            .try_into()
            .unwrap();
        let enc = &page[SALT_SIZE..PAGE_SIZE - RESERVE_SIZE];
        if let Ok(dec) = aes_cbc_decrypt(aes_key, iv, enc)
            && dec.len() >= 16
            && &dec[..15] == b"SQLite format 3"
        {
            return true;
        }

        // Fallback: PBKDF2-derive the raw key with the DB's salt (last 32 chars of 96-char key)
        #[cfg(target_os = "macos")]
        if key_hex.len() >= 96 {
            let key_part = &key_hex[..64];
            let salt_part = &key_hex[64..96];
            if let Some(derived) = pbkdf2_derive_openssl(key_part, salt_part)
                && derived != *aes_key
                && let Ok(dec) = aes_cbc_decrypt(&derived, iv, enc)
                && dec.len() >= 16
                && &dec[..15] == b"SQLite format 3"
            {
                return true;
            }
        }
    }
    false
}

fn decrypt_with_key(
    encrypted_path: &std::path::Path,
    raw_key_hex: &str,
    encrypted: &[u8],
) -> Result<PathBuf> {
    let key_bytes = hex_decode(&raw_key_hex[..64])
        .map_err(|e| QunMindError::Channel(format!("密钥 hex 解码失败: {e}")))?;
    let aes_key: &[u8; 32] = key_bytes[..32].try_into().unwrap();

    let temp_dir = std::env::temp_dir().join(format!("qunmind-wechat-{}", std::process::id()));
    fs::create_dir_all(&temp_dir)
        .map_err(|e| QunMindError::Channel(format!("无法创建临时目录: {e}")))?;
    let temp_path = temp_dir.join(
        encrypted_path
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("message.db")),
    );

    const IV_OFFSET: usize = PAGE_SIZE - RESERVE_SIZE;
    const DATA_END: usize = IV_OFFSET;
    const SQLITE_HDR: &[u8; 16] = b"SQLite format 3\x00";

    let page_count = encrypted.len() / PAGE_SIZE;
    let mut plaintext = Vec::with_capacity(page_count * PAGE_SIZE);

    for page_num in 0..page_count {
        let offset = page_num * PAGE_SIZE;
        let page = &encrypted[offset..offset + PAGE_SIZE];

        let iv: &[u8; 16] = page[IV_OFFSET..IV_OFFSET + 16].try_into().unwrap();
        let mut out = vec![0u8; PAGE_SIZE];

        if page_num == 0 {
            // Page 1: skip salt (first 16 bytes), decrypt [SALT_SIZE..DATA_END]
            let enc = &page[SALT_SIZE..DATA_END];
            let dec = aes_cbc_decrypt(aes_key, iv, enc)?;
            out[..16].copy_from_slice(SQLITE_HDR);
            out[16..DATA_END].copy_from_slice(&dec);
            // reserve area stays zero
        } else {
            // Page N: decrypt [0..DATA_END]
            let dec = aes_cbc_decrypt(aes_key, iv, &page[..DATA_END])?;
            out[..DATA_END].copy_from_slice(&dec);
        }

        plaintext.extend_from_slice(&out);
    }

    fs::write(&temp_path, &plaintext)
        .map_err(|e| QunMindError::Channel(format!("无法写入解密数据库: {e}")))?;

    debug!(
        path = %temp_path.display(),
        pages = page_count,
        "SQLCipher 数据库已解密"
    );

    Ok(temp_path)
}

/// Open a decrypted (plaintext) SQLite database.
pub fn open_decrypted_db(path: &std::path::Path) -> Result<rusqlite::Connection> {
    // We need to tell SQLite the page size is 4032, since we stripped the crypto overhead.
    let conn = rusqlite::Connection::open(path).map_err(|e| {
        QunMindError::Channel(format!("无法打开解密数据库 {}: {e}", path.display()))
    })?;

    // Check for valid SQLite header.
    let header_check: String = conn
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .unwrap_or_else(|_| "unknown".to_string());
    debug!(version = %header_check, "解密数据库已打开");

    Ok(conn)
}

fn detect_message_table(conn: &rusqlite::Connection) -> Result<String> {
    let tables = list_tables(conn);
    debug!(?tables, "数据库表列表");

    // WeChat 4.x candidates in preference order.
    for candidate in &["ChatMessage", "message", "msg", "Message", "messages"] {
        if tables.iter().any(|t| t == candidate) {
            return Ok((*candidate).to_string());
        }
    }
    Err(QunMindError::Channel(format!(
        "未找到消息表，现有表: {:?}",
        tables
    )))
}

// ---------------------------------------------------------------------------
// Persistent key cache
// ---------------------------------------------------------------------------

fn key_cache_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".qunmind").join("db_keys.cache")
}

/// Save extracted keys to disk so they survive binary rebuilds.
pub fn save_keys(keys: &[String]) {
    let path = key_cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::write(&path, keys.join("\n")) {
        Ok(_) => info!(path = %path.display(), count = keys.len(), "密钥已持久化缓存"),
        Err(e) => warn!(path = %path.display(), "密钥缓存写入失败: {e}"),
    }
}

/// Load previously cached keys from disk.
pub fn load_cached_keys() -> Vec<String> {
    let path = key_cache_path();
    fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect()
}

fn hex_decode(hex: &str) -> Result<Vec<u8>> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| QunMindError::Channel("无效的十六进制密钥".to_string()))
}

// ---------------------------------------------------------------------------
// Message querying
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RawWechatMessage {
    pub local_id: i64,
    pub svr_id: i64,
    pub create_time: i64,
    pub talker: String,
    pub content: String,
    pub msg_type: i64,
    pub is_sender: bool,
    /// Original string ID for captured JSON replay (empty for DB messages).
    pub original_id: String,
}

/// List all table names in the database (for schema diagnosis).
pub fn list_tables(conn: &rusqlite::Connection) -> Vec<String> {
    let mut stmt =
        match conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
    stmt.query_map([], |row| row.get(0))
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

pub fn query_new_messages(
    conn: &rusqlite::Connection,
    since_local_id: i64,
    limit: usize,
) -> Result<Vec<RawWechatMessage>> {
    // Detect message table name — WeChat 4.x may use different table names.
    let table = detect_message_table(conn)?;
    debug!(table, "使用消息表");

    let sql = format!(
        "SELECT local_id, svr_id, CreateTime, Talker, Message, Type, Des
         FROM {table}
         WHERE local_id > ?1
         ORDER BY local_id ASC
         LIMIT ?2"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| QunMindError::Channel(format!("查询微信消息失败 (table={table}): {e}")))?;

    let rows = stmt
        .query_map(rusqlite::params![since_local_id, limit as i64], |row| {
            Ok(RawWechatMessage {
                local_id: row.get(0)?,
                svr_id: row.get(1)?,
                create_time: row.get(2)?,
                talker: row.get(3)?,
                content: row.get(4)?,
                msg_type: row.get(5)?,
                is_sender: row.get::<_, i64>(6).unwrap_or(0) == 1,
                original_id: String::new(),
            })
        })
        .map_err(|e| QunMindError::Channel(format!("读取微信消息失败: {e}")))?;

    let mut messages = Vec::new();
    for row in rows {
        match row {
            Ok(msg) => messages.push(msg),
            Err(e) => warn!("跳过损坏的微信消息行: {e}"),
        }
    }

    Ok(messages)
}

// ---------------------------------------------------------------------------
// Read-only pipeline probe
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize)]
pub struct TableInfo {
    pub name: String,
    pub rows: i64,
    pub sql: String,
}

#[derive(Debug, serde::Serialize)]
pub struct DbProbe {
    pub path: String,
    pub salt: String,
    pub decrypted: bool,
    pub error: Option<String>,
    pub tables: Vec<TableInfo>,
}

#[derive(Debug, serde::Serialize)]
pub struct ProbeReport {
    pub candidate_keys: usize,
    pub key_source: String,
    pub total_db_shards: usize,
    pub databases: Vec<DbProbe>,
}

fn db_salt_hex(path: &std::path::Path) -> String {
    use std::io::Read;
    let mut f = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut salt = [0u8; SALT_SIZE];
    if f.read_exact(&mut salt).is_ok() {
        salt.iter().map(|b| format!("{b:02x}")).collect()
    } else {
        String::new()
    }
}

fn table_schemas(conn: &rusqlite::Connection) -> Vec<TableInfo> {
    let mut stmt = match conn
        .prepare("SELECT name, sql FROM sqlite_master WHERE type='table' ORDER BY name")
    {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let defs: Vec<(String, String)> = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        ))
    }) {
        Ok(rows) => rows.flatten().collect(),
        Err(_) => return Vec::new(),
    };
    drop(stmt);

    defs.into_iter()
        .map(|(name, sql)| {
            let quoted = name.replace('"', "\"\"");
            let rows = conn
                .query_row(&format!("SELECT count(*) FROM \"{quoted}\""), [], |r| {
                    r.get(0)
                })
                .unwrap_or(-1);
            TableInfo { name, rows, sql }
        })
        .collect()
}

/// Read-only diagnostic: resolve keys, decrypt the newest message shard,
/// and dump its real table schema. Touches no AI / PostgreSQL / send paths.
///
/// Lets the caller see exactly where the receive pipeline breaks:
/// `candidate_keys == 0` → key extraction; `decrypted == false` → decryption
/// (key mismatch); decrypted but no usable message table → 4.x schema.
pub fn probe() -> ProbeReport {
    let (keys, key_source) = match extract_all_db_keys() {
        Ok(k) => {
            save_keys(&k);
            (k, "memory_scan")
        }
        Err(_) => {
            let cached = load_cached_keys();
            let src = if cached.is_empty() {
                "none"
            } else {
                "disk_cache"
            };
            (cached, src)
        }
    };

    let shards = message_db_paths();
    let mut databases = Vec::new();
    if let Some(path) = shards.first() {
        let mut entry = DbProbe {
            path: path.display().to_string(),
            salt: db_salt_hex(path),
            decrypted: false,
            error: None,
            tables: Vec::new(),
        };
        match decrypt_db(path, &keys) {
            Ok(plain) => {
                match open_decrypted_db(&plain) {
                    Ok(conn) => {
                        entry.decrypted = true;
                        entry.tables = table_schemas(&conn);
                    }
                    Err(e) => entry.error = Some(e.to_string()),
                }
                // Decrypted plaintext contains private chats — never leave it on disk.
                let _ = fs::remove_file(&plain);
            }
            Err(e) => entry.error = Some(e.to_string()),
        }
        databases.push(entry);
    }

    ProbeReport {
        candidate_keys: keys.len(),
        key_source: key_source.to_string(),
        total_db_shards: shards.len(),
        databases,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_decode_parses_key() {
        let key = "00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff00ff";
        let bytes = hex_decode(key).unwrap();
        assert_eq!(bytes.len(), 48);
        assert_eq!(bytes[0], 0x00);
        assert_eq!(bytes[1], 0xff);
    }

    #[test]
    fn hex_decode_rejects_invalid() {
        assert!(hex_decode("xyz").is_err());
    }

    #[test]
    fn message_db_paths_filters_and_sorts_shards() {
        let dir =
            std::env::temp_dir().join(format!("qunmind-wechat-db-paths-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        for name in [
            "message_0.db",
            "message_12.db",
            "message_2.db",
            "message_2.db-wal",
            "message_7_fts.db",
            "message_8_resource.db",
            "message_a.db",
            "other.db",
        ] {
            fs::write(dir.join(name), "").unwrap();
        }

        let paths = message_db_paths_in_dir(&dir);
        let names: Vec<String> = paths
            .iter()
            .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
            .map(|name| name.to_string())
            .collect();

        assert_eq!(names, vec!["message_12.db", "message_2.db", "message_0.db"]);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn table_schemas_lists_tables_and_rows() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE ChatMessage(local_id INTEGER, Message TEXT);
             INSERT INTO ChatMessage VALUES (1, 'hi'), (2, 'yo');",
        )
        .unwrap();
        let infos = table_schemas(&conn);
        let cm = infos
            .iter()
            .find(|t| t.name == "ChatMessage")
            .expect("ChatMessage table present");
        assert_eq!(cm.rows, 2);
        assert!(cm.sql.contains("Message"));
    }
}
