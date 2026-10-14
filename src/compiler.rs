use crate::protocol::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    TtsSynthesis,
    SpeechAlignment,
    VisualGeneration,
    CaptionRender,
    AudioTrackMix,
    VideoComposition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTask {
    pub id: String,
    pub task_type: TaskType,
    pub title: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub estimated_duration_ms: u64,
    pub parameters: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub title: String,
    pub total_estimated_duration_ms: u64,
    pub total_speech_duration_ms: u64,
    pub tasks: Vec<PlanTask>,
    pub dependency_graph: HashMap<String, Vec<String>>,
    pub speakers: Vec<String>,
    pub sections_count: usize,
    pub lines_count: usize,
}

pub struct SvmlCompiler;

impl SvmlCompiler {
    pub fn compile(doc: &SvmlDocument, project_title: Option<&str>) -> ExecutionPlan {
        let mut tasks = Vec::new();
        let mut dependency_graph = HashMap::new();
        let mut speakers_set = std::collections::BTreeSet::new();

        let mut total_speech_duration_ms: u64 = 0;
        let mut line_index = 0;
        let mut prev_tts_task: Option<String> = None;
        let mut all_tts_tasks = Vec::new();
        let mut all_visual_tasks = Vec::new();
        let mut all_caption_tasks = Vec::new();

        for section in &doc.scripts {
            for line in &section.lines {
                line_index += 1;
                speakers_set.insert(line.speaker.clone());

                // Calculate estimated speech duration: ~150 words per minute -> 2.5 words/sec -> 400ms per word
                let words = line.clean_speech_text.split_whitespace().count().max(1);
                let speech_duration_ms = (words as u64 * 380) + (line.pauses_count as u64 * 300);
                total_speech_duration_ms += speech_duration_ms;

                // 1. TTS Synthesis Task
                let tts_id = format!("tts_{}_{}", section.name, line_index);
                let mut tts_params = BTreeMap::new();
                tts_params.insert("speaker".to_string(), line.speaker.clone());
                tts_params.insert("text".to_string(), line.clean_speech_text.clone());
                tts_params.insert("section".to_string(), section.name.clone());

                let mut tts_deps = Vec::new();
                if let Some(prev) = &prev_tts_task {
                    // Chain audio order for sequential timeline speech
                    tts_deps.push(prev.clone());
                }

                tasks.push(PlanTask {
                    id: tts_id.clone(),
                    task_type: TaskType::TtsSynthesis,
                    title: format!("TTS [{}] Line {}", line.speaker, line_index),
                    description: format!("Synthesize voice for {}: \"{}\"", line.speaker, line.clean_speech_text),
                    dependencies: tts_deps.clone(),
                    estimated_duration_ms: speech_duration_ms,
                    parameters: tts_params,
                });
                dependency_graph.insert(tts_id.clone(), tts_deps);
                all_tts_tasks.push(tts_id.clone());
                prev_tts_task = Some(tts_id.clone());

                // 2. Speech-to-Text Alignment Task
                let align_id = format!("align_{}_{}", section.name, line_index);
                let align_deps = vec![tts_id.clone()];
                tasks.push(PlanTask {
                    id: align_id.clone(),
                    task_type: TaskType::SpeechAlignment,
                    title: format!("Align Line {}", line_index),
                    description: format!("Forced phoneme & word alignment via WhisperX/VAD"),
                    dependencies: align_deps.clone(),
                    estimated_duration_ms: 100,
                    parameters: BTreeMap::new(),
                });
                dependency_graph.insert(align_id.clone(), align_deps);

                // 3. Caption Render Task
                let caption_id = format!("caption_{}_{}", section.name, line_index);
                let caption_deps = vec![align_id];
                tasks.push(PlanTask {
                    id: caption_id.clone(),
                    task_type: TaskType::CaptionRender,
                    title: format!("Caption Line {}", line_index),
                    description: format!("Generate styled kinetic subtitles for line {}", line_index),
                    dependencies: caption_deps.clone(),
                    estimated_duration_ms: 50,
                    parameters: BTreeMap::new(),
                });
                dependency_graph.insert(caption_id.clone(), caption_deps);
                all_caption_tasks.push(caption_id);

                // 4. Visual Generation for cues
                for cue in &line.cues {
                    if !cue.is_closing {
                        let visual_id = format!("visual_{}_{}_{}", section.name, line_index, cue.name);
                        let mut vis_params = BTreeMap::new();
                        vis_params.insert("cue".to_string(), cue.name.clone());
                        vis_params.insert("section".to_string(), section.name.clone());

                        tasks.push(PlanTask {
                            id: visual_id.clone(),
                            task_type: TaskType::VisualGeneration,
                            title: format!("Visual Cue: @{{{}}}", cue.name),
                            description: format!("Generate or extract B-roll asset for cue '{}'", cue.name),
                            dependencies: Vec::new(),
                            estimated_duration_ms: 1500,
                            parameters: vis_params,
                        });
                        dependency_graph.insert(visual_id.clone(), Vec::new());
                        all_visual_tasks.push(visual_id);
                    }
                }
            }
        }

        // 5. Audio Mix Task
        let audio_mix_id = "audio_master_mix".to_string();
        tasks.push(PlanTask {
            id: audio_mix_id.clone(),
            task_type: TaskType::AudioTrackMix,
            title: "Master Audio Mix".to_string(),
            description: "Concatenate dialog tracks, apply background music ducking and normalization".to_string(),
            dependencies: all_tts_tasks.clone(),
            estimated_duration_ms: 500,
            parameters: BTreeMap::new(),
        });
        dependency_graph.insert(audio_mix_id.clone(), all_tts_tasks);

        // 6. Final Video Composition Task
        let mut render_deps = vec![audio_mix_id];
        render_deps.extend(all_visual_tasks);
        render_deps.extend(all_caption_tasks);

        let final_task_id = "video_render_final".to_string();
        tasks.push(PlanTask {
            id: final_task_id.clone(),
            task_type: TaskType::VideoComposition,
            title: "Final Video Assembly".to_string(),
            description: "Composite visual layers, overlay kinetic captions, and encode final MP4 via FFmpeg".to_string(),
            dependencies: render_deps.clone(),
            estimated_duration_ms: (total_speech_duration_ms / 2).max(2000),
            parameters: BTreeMap::new(),
        });
        dependency_graph.insert(final_task_id, render_deps);

        let total_est = tasks.iter().map(|t| t.estimated_duration_ms).sum::<u64>();

        ExecutionPlan {
            title: project_title.unwrap_or("SVML Video Project").to_string(),
            total_estimated_duration_ms: total_est,
            total_speech_duration_ms,
            tasks,
            dependency_graph,
            speakers: speakers_set.into_iter().collect(),
            sections_count: doc.scripts.len(),
            lines_count: line_index,
        }
    }
}
