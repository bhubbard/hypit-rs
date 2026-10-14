use crate::compiler::SvmlCompiler;
use crate::kernel::{KernelEngine, TaskStatus};
use crate::markup::SvmlParser;
use crate::runtime::MediaRuntime;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "hypit")]
#[command(author = "Brandon Hubbard & hypit-rs contributors")]
#[command(version = "0.1.0")]
#[command(about = "High-performance Rust engine for Hypit: clone viral videos with AI agents and SVML", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Check and validate an SVML source file for syntax and structure
    Check {
        /// Path to the .svml file
        source: PathBuf,
    },

    /// Compile SVML into an execution plan and display the dependency DAG
    Plan {
        /// Path to the .svml file
        source: PathBuf,

        /// Output the plan in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Parse an SVML file and print its structured AST as JSON
    Parse {
        /// Path to the .svml file
        source: PathBuf,
    },

    /// Execute the video generation pipeline from an SVML source
    Run {
        /// Path to the .svml file
        source: PathBuf,

        /// Output video file path (.mp4)
        #[arg(short, long, default_value = "output.mp4")]
        output: PathBuf,

        /// Skip video encoding and only process plan/cache
        #[arg(long)]
        dry_run: bool,
    },

    /// Display environment info, detected FFmpeg encoders and hardware acceleration
    Info,
}

impl Cli {
    pub async fn execute(self) -> Result<()> {
        match self.command {
            Commands::Check { source } => check_source(&source),
            Commands::Plan { source, json } => plan_source(&source, json),
            Commands::Parse { source } => parse_source(&source),
            Commands::Run { source, output, dry_run } => run_source(&source, &output, dry_run).await,
            Commands::Info => show_info(),
        }
    }
}

fn check_source(path: &Path) -> Result<()> {
    println!("{}", format!("🔍 Checking SVML source: {}", path.display()).bold());
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let parser = SvmlParser::new(&content);
    let doc = parser.parse()
        .with_context(|| format!("Syntax error while parsing {}", path.display()))?;

    let lines_count: usize = doc.scripts.iter().map(|s| s.lines.len()).sum();
    let mut speakers: Vec<String> = doc.scripts.iter()
        .flat_map(|s| s.lines.iter().map(|l| l.speaker.clone()))
        .collect();
    speakers.sort();
    speakers.dedup();

    println!("{}", "✓ SVML Syntax Valid!".green().bold());
    if let Some(ref using) = doc.using {
        println!("  • Using: {}", using.cyan());
    }
    println!("  • Imports: {} package(s)", doc.imports.len());
    println!("  • Sections: {} script section(s)", doc.scripts.len());
    println!("  • Dialog Lines: {}", lines_count);
    println!("  • Speakers: {}", speakers.join(", ").yellow());
    println!("  • Text Prompts: {}", doc.text_values.len());

    Ok(())
}

fn plan_source(path: &Path, as_json: bool) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let parser = SvmlParser::new(&content);
    let doc = parser.parse()?;
    let title = path.file_stem().and_then(|s| s.to_str());
    let plan = SvmlCompiler::compile(&doc, title);

    if as_json {
        let json_str = serde_json::to_string_pretty(&plan)?;
        println!("{}", json_str);
        return Ok(());
    }

    println!("\n{}", format!("📋 Execution Plan for: {}", plan.title).bold().underline());
    println!("  Estimated Video Speech: {}s", format!("{:.1}", plan.total_speech_duration_ms as f64 / 1000.0).yellow());
    println!("  Total Tasks in DAG: {}", plan.tasks.len().to_string().cyan());
    println!("  Cast: {}\n", plan.speakers.join(", ").green());

    println!("{:<28} {:<18} {:<10} {}", "TASK ID", "TYPE", "EST. DURATION", "DEPENDS ON");
    println!("{}", "-".repeat(80));

    for task in &plan.tasks {
        let deps_str = if task.dependencies.is_empty() {
            "none".dimmed().to_string()
        } else {
            task.dependencies.join(", ").dimmed().to_string()
        };

        let type_name = match task.task_type {
            crate::compiler::TaskType::TtsSynthesis => "TTS".magenta(),
            crate::compiler::TaskType::SpeechAlignment => "Alignment".blue(),
            crate::compiler::TaskType::VisualGeneration => "Visual".cyan(),
            crate::compiler::TaskType::CaptionRender => "Caption".yellow(),
            crate::compiler::TaskType::AudioTrackMix => "AudioMix".bright_purple(),
            crate::compiler::TaskType::VideoComposition => "Render".bright_green(),
        };

        println!(
            "{:<28} {:<27} {:<10} {}",
            task.id,
            type_name,
            format!("{}ms", task.estimated_duration_ms),
            deps_str
        );
    }
    println!();

    Ok(())
}

fn parse_source(path: &Path) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let parser = SvmlParser::new(&content);
    let doc = parser.parse()?;

    let json_str = serde_json::to_string_pretty(&doc)?;
    println!("{}", json_str);
    Ok(())
}

async fn run_source(path: &Path, output: &Path, dry_run: bool) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let parser = SvmlParser::new(&content);
    let doc = parser.parse()?;
    let title = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Hypit Video");
    let plan = SvmlCompiler::compile(&doc, Some(title));

    let project_root = path.parent().unwrap_or(Path::new("."));
    let kernel = KernelEngine::new(project_root);

    println!("{}", format!("🚀 Running Pipeline: {} ({} tasks)", title, plan.tasks.len()).bold().cyan());

    kernel.execute(&plan, |task, status| {
        match status {
            TaskStatus::Running => {
                print!("  [..] {:<32} ... ", task.title.dimmed());
                std::io::Write::flush(&mut std::io::stdout()).ok();
            }
            TaskStatus::Completed { cached, .. } => {
                if *cached {
                    println!("{}", "CACHED".bright_blue().bold());
                } else {
                    println!("{}", "OK".green().bold());
                }
            }
            TaskStatus::Failed(err) => {
                println!("{} {}", "FAILED:".red().bold(), err);
            }
            _ => {}
        }
    }).await?;

    if !dry_run {
        println!("{}", format!("🎬 Encoding final video to: {}", output.display()).bold());
        let duration_secs = (plan.total_speech_duration_ms / 1000).max(3);
        MediaRuntime::render_placeholder_video(output, duration_secs, title)?;
        println!("{}", format!("🎉 Successfully rendered video: {}", output.display()).green().bold());
    } else {
        println!("{}", "ℹ Dry-run enabled: skipped FFmpeg encoding step.".yellow());
    }

    Ok(())
}

fn show_info() -> Result<()> {
    let caps = MediaRuntime::detect_capabilities();
    println!("{}", "⚡ Hypit-rs Engine Info".bold().underline());
    println!("  • Version: {}", env!("CARGO_PKG_VERSION").green());
    println!("  • Architecture: Pure Rust 2021 Edition");

    println!("\n{}", "🎥 Media Processing Runtime:".bold());
    match caps.ffmpeg_path {
        Some(path) => {
            println!("  • FFmpeg: {}", path.display().to_string().cyan());
            if let Some(v) = caps.ffmpeg_version {
                println!("    {}", v.dimmed());
            }
        }
        None => println!("  • FFmpeg: {}", "Not found (required for video rendering)".red()),
    }

    println!("\n{}", "🚀 Hardware Acceleration:".bold());
    println!("  • Apple VideoToolbox (macOS): {}", if caps.has_videotoolbox { "Available ✓".green() } else { "No".dimmed() });
    println!("  • NVIDIA NVENC: {}", if caps.has_nvenc { "Available ✓".green() } else { "No".dimmed() });

    Ok(())
}
