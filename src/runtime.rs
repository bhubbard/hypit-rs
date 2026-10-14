use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct RuntimeCapabilities {
    pub ffmpeg_path: Option<PathBuf>,
    pub ffprobe_path: Option<PathBuf>,
    pub ffmpeg_version: Option<String>,
    pub has_videotoolbox: bool,
    pub has_nvenc: bool,
}

pub struct MediaRuntime;

impl MediaRuntime {
    pub fn detect_capabilities() -> RuntimeCapabilities {
        let ffmpeg_path = which::which("ffmpeg").or_else(|_| {
            let default_paths = ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg", "/usr/bin/ffmpeg"];
            default_paths.iter().map(PathBuf::from).find(|p| p.exists()).ok_or(())
        }).ok();

        let ffprobe_path = which::which("ffprobe").or_else(|_| {
            let default_paths = ["/opt/homebrew/bin/ffprobe", "/usr/local/bin/ffprobe", "/usr/bin/ffprobe"];
            default_paths.iter().map(PathBuf::from).find(|p| p.exists()).ok_or(())
        }).ok();

        let mut ffmpeg_version = None;
        let mut has_videotoolbox = false;
        let mut has_nvenc = false;

        if let Some(ref path) = ffmpeg_path {
            if let Ok(output) = Command::new(path).arg("-version").output() {
                let out_str = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = out_str.lines().next() {
                    ffmpeg_version = Some(first_line.to_string());
                }
            }

            if let Ok(output) = Command::new(path).args(["-encoders"]).output() {
                let out_str = String::from_utf8_lossy(&output.stdout);
                has_videotoolbox = out_str.contains("h264_videotoolbox");
                has_nvenc = out_str.contains("h264_nvenc");
            }
        }

        RuntimeCapabilities {
            ffmpeg_path,
            ffprobe_path,
            ffmpeg_version,
            has_videotoolbox,
            has_nvenc,
        }
    }

    pub fn render_placeholder_video<P: AsRef<Path>>(
        output_path: P,
        duration_secs: u64,
        title: &str,
    ) -> Result<()> {
        let caps = Self::detect_capabilities();
        let ffmpeg = match caps.ffmpeg_path {
            Some(p) => p,
            None => bail!("FFmpeg is not installed. Please install FFmpeg to render video files."),
        };

        let duration_str = format!("{}", duration_secs);
        let escaped_title = title.replace('\'', "\\'").replace(':', "\\:");
        let drawtext = format!(
            "drawtext=text='{}':fontcolor=white:fontsize=48:box=1:boxcolor=black@0.6:boxborderw=10:x=(w-text_w)/2:y=(h-text_h)/2",
            escaped_title
        );

        let encoder = if caps.has_videotoolbox {
            "h264_videotoolbox"
        } else {
            "libx264"
        };

        // Generates a 1080x1920 (9:16 vertical video) test pattern with title text
        let status = Command::new(ffmpeg)
            .args([
                "-y",
                "-f", "lavfi",
                "-i", &format!("color=c=0x111318:s=1080x1920:d={}", duration_str),
                "-f", "lavfi",
                "-i", &format!("anullsrc=r=44100:cl=stereo:d={}", duration_str),
                "-vf", &drawtext,
                "-c:v", encoder,
                "-pix_fmt", "yuv420p",
                "-c:a", "aac",
                "-shortest",
                output_path.as_ref().to_str().unwrap(),
            ])
            .output()?;

        if !status.status.success() {
            let err = String::from_utf8_lossy(&status.stderr);
            bail!("FFmpeg rendering failed: {}", err);
        }

        Ok(())
    }
}

// Minimal fallback for which without external dependency
mod which {
    use std::path::PathBuf;
    pub fn which(binary_name: &str) -> Result<PathBuf, ()> {
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let candidate = dir.join(binary_name);
                if candidate.is_file() && candidate.exists() {
                    return Ok(candidate);
                }
            }
        }
        Err(())
    }
}
