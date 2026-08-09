//! 图片容器解析与元数据提取。
//!
//! 核心目标：不在此处解码任何像素 —— 所有像素级工作交给前端浏览器引擎
//! （Chromium / WebKit），它们的高质量缩放、ICC 色彩管理、Ultra HDR 渲染
//! 远优于在 Rust 侧用 Pillow/自研代码实现。
//!
//! Rust 侧只做：容器切分（JPEG / gain map / MP4）、EXIF 元数据、尺寸、
//! Ultra HDR 检测、视频旋转角读取。

use serde::Serialize;
use std::io::Read;
use std::path::Path;

/// 支持的图片扩展名（小写）。
pub const SUPPORTED_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "avif", "gif", "bmp"];

/// Ultra HDR（Android 增益图）元数据。
///
/// 注意：SDR 屏幕上**不应用** gain map（与 Chrome 行为一致），仅用于
/// 徽标展示与信息显示。
#[derive(Serialize, Clone, Default)]
pub struct UltraHdrMeta {
    pub gain_map_min: Option<f64>,
    pub gain_map_max: Option<f64>,
    pub gamma: Option<f64>,
    /// Adobe XMP `hdrgm` 命名空间（本机测试文件采用此方案）。
    pub has_xmp_hdrgm: bool,
    /// ISO 21496-1 标准（Safari/Chromium 优先读取）。
    pub has_iso_21496: bool,
}

#[derive(Serialize, Clone)]
pub struct PhotoMeta {
    pub id: u64,
    pub path: String,
    pub name: String,
    /// 存储的原始像素尺寸（未应用 EXIF 旋转）。
    pub width: u32,
    pub height: u32,
    /// EXIF Orientation (1-8)。
    pub orientation: u16,
    /// 是否内嵌 MP4（Live Photo）。
    pub is_live: bool,
    /// MP4 起始偏移（若在扫描缓冲区内发现）。
    pub mp4_offset: Option<u64>,
    /// 内嵌视频的旋转角（0/90/180/270）。
    pub video_rotation: u16,
    pub size: u64,
    /// DateTimeOriginal，字符串形式。
    pub date: Option<String>,
    pub ultra_hdr: Option<UltraHdrMeta>,
}

pub fn is_supported(p: &Path) -> bool {
    match p.extension().and_then(|e| e.to_str()) {
        Some(e) => {
            let ext = e.to_ascii_lowercase();
            SUPPORTED_EXTS.contains(&ext.as_str())
        }
        None => false,
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn rfind_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let mut i = haystack.len() - needle.len();
    loop {
        if &haystack[i..i + needle.len()] == needle {
            return Some(i);
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    None
}

/// 找到 MP4 起始偏移。MP4 的 `ftyp` box 位于 `ftypmp42`/`ftypisom` 标记前 4 字节
/// （4 字节 box size 前缀）。返回 box 起始偏移。
fn find_mp4_offset(buf: &[u8]) -> Option<usize> {
    if let Some(pos) = find_bytes(buf, b"ftypmp42") {
        return pos.checked_sub(4);
    }
    find_bytes(buf, b"ftypisom").and_then(|pos| pos.checked_sub(4))
}

fn xmp_number(xmp: &str, key: &str) -> Option<f64> {
    let pat = format!("{key}=\"");
    let i = xmp.find(&pat)?;
    let rest = &xmp[i + pat.len()..];
    let end = rest.find('"')?;
    rest[..end].trim().parse().ok()
}

/// 从 MP4 的 tkhd matrix 解析旋转角。启发式：在字节流中找第一个 `tkhd`。
fn parse_video_rotation(mp4: &[u8]) -> u16 {
    let Some(pos) = find_bytes(mp4, b"tkhd") else {
        return 0;
    };
    let version = *mp4.get(pos + 4).unwrap_or(&0);
    // tkhd 版本 0：matrix 在 box 头后偏移 48；版本 1：偏移 60
    let matrix_off = if version == 1 { 60 } else { 48 };
    let m = pos + 4 + matrix_off;
    if m + 36 > mp4.len() {
        return 0;
    }
    let read_i32 = |i: usize| -> i32 {
        i32::from_be_bytes([mp4[m + i], mp4[m + i + 1], mp4[m + i + 2], mp4[m + i + 3]])
    };
    // matrix = [a b u; c d v; x y w]，a/c 与 b/d 表示旋转
    let a = read_i32(0) as f64 / 65536.0;
    let b = read_i32(4) as f64 / 65536.0;
    let deg = b.atan2(a).to_degrees();
    let rounded = ((deg + 45.0) / 90.0).floor() * 90.0;
    (rounded as i32).rem_euclid(360) as u16
}

fn read_exif(path: &Path) -> (u16, Option<String>) {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return (1, None),
    };
    let mut reader = std::io::BufReader::new(file);
    let exif_reader = exif::Reader::new();
    let exif = match exif_reader.read_from_container(&mut reader) {
        Ok(e) => e,
        Err(_) => return (1, None),
    };
    let orientation = exif
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .unwrap_or(1)
        .min(8) as u16;
    let date = exif
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .and_then(|f| {
            std::panic::catch_unwind(|| f.value.display_as(exif::Tag::DateTimeOriginal).to_string())
                .ok()
        });
    (orientation, date)
}

/// 扫描文件夹时使用：只读有限字节（≤8MB）拿到所有元数据，避免全量读大文件。
pub fn scan_photo(path: &Path, id: u64) -> std::io::Result<PhotoMeta> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let size = std::fs::metadata(path)?.len();

    let (width, height) = imagesize::size(path)
        .map(|s| (s.width as u32, s.height as u32))
        .unwrap_or((0, 0));
    let (orientation, date) = read_exif(path);

    // 分块读取，上限 8MB。XMP/EXIF 在文件开头，`ftyp` 标记通常在视频嵌入处。
    const CAP: usize = 8 * 1024 * 1024;
    let mut buf = Vec::new();
    {
        let mut f = std::fs::File::open(path)?;
        let mut chunk = vec![0u8; 1024 * 1024];
        loop {
            let n = f.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            if buf.len() >= CAP {
                break;
            }
        }
    }

    // Live Photo 检测：XMP GContainer 列出 MotionPhoto 条目，或发现 ftyp 标记。
    let mut is_live = find_bytes(&buf, b"MotionPhoto").is_some();
    let mp4_offset = find_mp4_offset(&buf);
    if mp4_offset.is_some() {
        is_live = true;
    }
    let video_rotation = match mp4_offset {
        Some(off) => parse_video_rotation(&buf[off..]),
        None => 0,
    };

    // Ultra HDR 检测 + gain map 参数提取。
    // 主图 XMP 在文件开头声明 hdrgm 命名空间；gain map 参数（GainMapMax 等）
    // 位于 gain map JPEG 自身的 XMP 中，因此在整个扫描缓冲区内搜索。
    let mut uh = UltraHdrMeta::default();
    uh.has_xmp_hdrgm = find_bytes(&buf, b"hdrgm").is_some();
    uh.has_iso_21496 = find_bytes(&buf, b"GainMapVersion").is_some();
    if uh.has_xmp_hdrgm || uh.has_iso_21496 {
        let text = String::from_utf8_lossy(&buf);
        uh.gain_map_min = xmp_number(&text, "GainMapMin");
        uh.gain_map_max = xmp_number(&text, "GainMapMax");
        uh.gamma = xmp_number(&text, "Gamma");
    }
    let ultra_hdr = if uh.has_xmp_hdrgm || uh.has_iso_21496 {
        Some(uh)
    } else {
        None
    };

    Ok(PhotoMeta {
        id,
        path: path.to_string_lossy().into_owned(),
        name,
        width,
        height,
        orientation,
        is_live,
        mp4_offset: mp4_offset.map(|o| o as u64),
        video_rotation,
        size,
        date,
        ultra_hdr,
    })
}

/// 打开单张图片时使用：全量读取并精确切分为 JPEG 部分与 MP4 部分。
/// JPEG 部分（含 Ultra HDR gain map 与 XMP/MPF）直接交给浏览器，
/// 浏览器会自行决定 SDR / HDR 渲染。
pub fn split_file(path: &Path) -> std::io::Result<(Vec<u8>, Option<Vec<u8>>)> {
    let data = std::fs::read(path)?;
    match find_mp4_offset(&data) {
        Some(off) => {
            let mut jpeg = data[..off].to_vec();
            // 剪掉 gain map EOI 之后的零填充，得到干净的 JPEG。
            if let Some(eoi) = rfind_bytes(&jpeg, &[0xFF, 0xD9]) {
                jpeg.truncate(eoi + 2);
            }
            Ok((jpeg, Some(data[off..].to_vec())))
        }
        None => Ok((data, None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_FILE: &str = "/home/yezichao/.openclaw/workspace/20260227-144315.live.jpeg";

    #[test]
    fn scan_parses_ultra_hdr_live_photo() {
        let path = Path::new(TEST_FILE);
        if !path.exists() {
            eprintln!("跳过：测试文件不存在");
            return;
        }
        let meta = scan_photo(path, 42).expect("scan_photo 失败");
        assert_eq!(meta.width, 3456);
        assert_eq!(meta.height, 4608);
        assert!(meta.is_live, "应检测为 Live Photo");
        assert!(meta.mp4_offset.is_some());
        let uh = meta.ultra_hdr.expect("应检测到 Ultra HDR");
        assert!(uh.has_xmp_hdrgm);
        assert!(uh.gain_map_max.is_some(), "应提取到 GainMapMax");
        if let Some(max) = uh.gain_map_max {
            assert!((max - 1.26315).abs() < 0.001, "GainMapMax={max}");
        }
    }

    #[test]
    fn split_yields_jpeg_and_mp4() {
        let path = Path::new(TEST_FILE);
        if !path.exists() {
            eprintln!("跳过：测试文件不存在");
            return;
        }
        let (jpeg, mp4) = split_file(path).expect("split_file 失败");
        let mp4 = mp4.expect("应切出 MP4");
        // JPEG 部分应为合法 JPEG（以 SOI 开头，以 EOI 结尾）
        assert!(jpeg.starts_with(&[0xFF, 0xD8]));
        assert!(jpeg.ends_with(&[0xFF, 0xD9]));
        // MP4 部分应包含 ftyp
        assert!(mp4.windows(4).any(|w| w == b"ftyp"));
        // 尺寸合理：JPEG 约 7.4MB，MP4 约 9.3MB
        assert!(jpeg.len() > 5_000_000);
        assert!(mp4.len() > 8_000_000);
    }
}
