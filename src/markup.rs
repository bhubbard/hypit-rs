use crate::protocol::*;
use anyhow::{bail, Context, Result};
use regex::Regex;
use std::collections::BTreeMap;

pub struct SvmlParser<'a> {
    source: &'a str,
    cursor: usize,
}

impl<'a> SvmlParser<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source, cursor: 0 }
    }

    pub fn parse(mut self) -> Result<SvmlDocument> {
        let mut doc = SvmlDocument {
            using: None,
            imports: Vec::new(),
            scripts: Vec::new(),
            text_values: BTreeMap::new(),
            root_elements: Vec::new(),
        };

        self.skip_trivia();

        // 1. Optional processing instruction: <?svml using="@hypit/markup@1"?>
        if self.starts_with("<?svml") {
            let end_pi = self.source[self.cursor..]
                .find("?>")
                .context("Unclosed <?svml processing instruction")?;
            let pi_text = &self.source[self.cursor..self.cursor + end_pi];
            if let Some(pos) = pi_text.find("using=") {
                let quote_char = pi_text.as_bytes().get(pos + 6).copied().unwrap_or(b'"') as char;
                let val_start = pos + 7;
                if let Some(quote_end) = pi_text[val_start..].find(quote_char) {
                    doc.using = Some(pi_text[val_start..val_start + quote_end].to_string());
                }
            }
            self.cursor += end_pi + 2;
            self.skip_trivia();
        }

        // Parse elements until EOF
        while self.cursor < self.source.len() {
            self.skip_trivia();
            if self.cursor >= self.source.len() {
                break;
            }

            if self.starts_with("<") {
                let el = self.parse_element()?;
                if el.name == "svml" {
                    // Extract children of svml root
                    for child in el.children {
                        match child {
                            SvmlNode::Element(inner) => {
                                if inner.name == "import" {
                                    doc.imports.push(Self::element_to_import(&inner));
                                } else if inner.name == "script" {
                                    doc.scripts.extend(Self::parse_script_raw(&inner.text_content()));
                                } else if inner.name == "text:Value" {
                                    if let Some(id) = inner.get_attr_str("id") {
                                        doc.text_values.insert(id.to_string(), inner.text_content());
                                    }
                                    doc.root_elements.push(inner);
                                } else {
                                    doc.root_elements.push(inner);
                                }
                            }
                            SvmlNode::Text { .. } => {}
                        }
                    }
                } else if el.name == "import" {
                    doc.imports.push(Self::element_to_import(&el));
                } else if el.name == "script" {
                    doc.scripts.extend(Self::parse_script_raw(&el.text_content()));
                } else {
                    doc.root_elements.push(el);
                }
            } else {
                // Unexpected trailing text
                break;
            }
        }

        Ok(doc)
    }

    fn skip_trivia(&mut self) {
        while self.cursor < self.source.len() {
            let rem = &self.source[self.cursor..];
            let ws_len = rem.chars().take_while(|c| c.is_whitespace()).map(|c| c.len_utf8()).sum::<usize>();
            self.cursor += ws_len;

            // Skip comments: <!-- ... -->
            if self.starts_with("<!--") {
                if let Some(end) = self.source[self.cursor..].find("-->") {
                    self.cursor += end + 3;
                } else {
                    self.cursor = self.source.len();
                    break;
                }
            } else {
                break;
            }
        }
    }

    fn starts_with(&self, s: &str) -> bool {
        self.source[self.cursor..].starts_with(s)
    }

    fn parse_element(&mut self) -> Result<SvmlElement> {
        let start = self.cursor;
        if !self.starts_with("<") {
            bail!("Expected '<' at offset {}", start);
        }
        self.cursor += 1;

        // Tag name
        let name_end = self.source[self.cursor..]
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .context("Unexpected EOF in tag name")?;
        let name = self.source[self.cursor..self.cursor + name_end].trim().to_string();
        self.cursor += name_end;

        let mut attributes = BTreeMap::new();

        // Attributes
        loop {
            self.skip_whitespace();
            if self.starts_with("/>") {
                self.cursor += 2;
                return Ok(SvmlElement {
                    name,
                    attributes,
                    children: Vec::new(),
                    range: self.calculate_range(start, self.cursor),
                });
            }
            if self.starts_with(">") {
                self.cursor += 1;
                break;
            }

            // Parse attribute key
            let attr_key_end = self.source[self.cursor..]
                .find(|c: char| c.is_whitespace() || c == '=' || c == '/' || c == '>')
                .context("Unexpected EOF in attribute key")?;
            let key = self.source[self.cursor..self.cursor + attr_key_end].trim().to_string();
            self.cursor += attr_key_end;
            self.skip_whitespace();

            if self.starts_with("=") {
                self.cursor += 1;
                self.skip_whitespace();
                let val = self.parse_attr_value()?;
                attributes.insert(key, val);
            } else {
                attributes.insert(key, AttributeValue::Literal("true".to_string()));
            }
        }

        // Special handling for <script> - avoid recursive XML parsing because speaker turns don't have closing tags
        if name == "script" {
            let script_close = "</script>";
            if let Some(close_pos) = self.source[self.cursor..].find(script_close) {
                let inner_raw = &self.source[self.cursor..self.cursor + close_pos];
                self.cursor += close_pos + script_close.len();

                return Ok(SvmlElement {
                    name,
                    attributes,
                    children: vec![SvmlNode::Text {
                        value: inner_raw.to_string(),
                        range: self.calculate_range(start, self.cursor),
                    }],
                    range: self.calculate_range(start, self.cursor),
                });
            }
        }

        // Parse children until closing tag: </name>
        let closing_tag = format!("</{}>", name);
        let mut children = Vec::new();

        while self.cursor < self.source.len() {
            self.skip_trivia();
            if self.starts_with(&closing_tag) {
                self.cursor += closing_tag.len();
                break;
            }

            if self.starts_with("</") {
                let end = self.source[self.cursor..].find('>').unwrap_or(closing_tag.len());
                self.cursor += end + 1;
                break;
            }

            if self.starts_with("<") {
                let child_el = self.parse_element()?;
                children.push(SvmlNode::Element(child_el));
            } else {
                let text_start = self.cursor;
                let next_tag = self.source[self.cursor..].find('<').unwrap_or(self.source.len() - self.cursor);
                let text = &self.source[self.cursor..self.cursor + next_tag];
                self.cursor += next_tag;

                if !text.trim().is_empty() {
                    children.push(SvmlNode::Text {
                        value: Self::decode_entities(text),
                        range: self.calculate_range(text_start, self.cursor),
                    });
                }
            }
        }

        Ok(SvmlElement {
            name,
            attributes,
            children,
            range: self.calculate_range(start, self.cursor),
        })
    }

    fn parse_attr_value(&mut self) -> Result<AttributeValue> {
        let first = self.source.as_bytes().get(self.cursor).copied().unwrap_or(0);
        if first == b'"' || first == b'\'' {
            let quote = first as char;
            self.cursor += 1;
            let end = self.source[self.cursor..]
                .find(quote)
                .context("Unclosed attribute string quote")?;
            let val = &self.source[self.cursor..self.cursor + end];
            self.cursor += end + 1;
            Ok(AttributeValue::Literal(Self::decode_entities(val)))
        } else if first == b'{' {
            self.cursor += 1;
            let end = self.source[self.cursor..]
                .find('}')
                .context("Unclosed reference attribute expression '{'")?;
            let val = self.source[self.cursor..self.cursor + end].trim().to_string();
            self.cursor += end + 1;
            Ok(AttributeValue::Reference(val))
        } else {
            let end = self.source[self.cursor..]
                .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .unwrap_or(self.source.len() - self.cursor);
            let val = &self.source[self.cursor..self.cursor + end];
            self.cursor += end;
            Ok(AttributeValue::Literal(val.to_string()))
        }
    }

    fn skip_whitespace(&mut self) {
        let len = self.source[self.cursor..]
            .chars()
            .take_while(|c| c.is_whitespace())
            .map(|c| c.len_utf8())
            .sum::<usize>();
        self.cursor += len;
    }

    fn decode_entities(val: &str) -> String {
        val.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
    }

    fn calculate_range(&self, start: usize, end: usize) -> SourceRange {
        let prefix = &self.source[..start];
        let line = prefix.lines().count().max(1);
        let last_line = prefix.rfind('\n').map(|p| start - p).unwrap_or(start + 1);
        SourceRange {
            start,
            end,
            line,
            column: last_line,
        }
    }

    fn element_to_import(el: &SvmlElement) -> SvmlImport {
        let as_alias = el.get_attr_str("as").map(|s| s.to_string());
        let from = el.get_attr_str("from").map(|s| s.to_string());
        let source = el.get_attr_str("source").map(|s| s.to_string());
        let kind = if source.is_some() {
            ImportKind::Source
        } else {
            ImportKind::Module
        };

        SvmlImport {
            kind,
            as_alias,
            from,
            source,
            range: el.range.clone(),
        }
    }

    pub fn parse_script_raw(raw_script: &str) -> Vec<ScriptSection> {
        let mut sections = Vec::new();
        let turn_re = Regex::new(r"<([A-Z0-9_-]{2,})>").unwrap();
        let seg_start_re = Regex::new(r"<([a-z0-9_-]+)>").unwrap();

        let mut cursor = 0;
        while cursor < raw_script.len() {
            if let Some(m) = seg_start_re.find(&raw_script[cursor..]) {
                let name = &m.as_str()[1..m.as_str().len() - 1];
                let content_start = cursor + m.end();
                let close_tag = format!("</{}>", name);

                if let Some(close_pos) = raw_script[content_start..].find(&close_tag) {
                    let section_content = &raw_script[content_start..content_start + close_pos];
                    cursor = content_start + close_pos + close_tag.len();

                    let mut lines = Vec::new();
                    let matches: Vec<_> = turn_re.find_iter(section_content).collect();

                    for (i, tm) in matches.iter().enumerate() {
                        let speaker = &tm.as_str()[1..tm.as_str().len() - 1];
                        let text_start = tm.end();
                        let text_end = if i + 1 < matches.len() {
                            matches[i + 1].start()
                        } else {
                            section_content.len()
                        };

                        let raw_dialog = section_content[text_start..text_end].trim();
                        if !raw_dialog.is_empty() {
                            let line = Self::parse_speaker_dialog(speaker, raw_dialog);
                            lines.push(line);
                        }
                    }

                    sections.push(ScriptSection {
                        name: name.to_string(),
                        lines,
                    });
                } else {
                    cursor += m.end();
                }
            } else {
                break;
            }
        }

        sections
    }

    fn parse_speaker_dialog(speaker: &str, raw_text: &str) -> ScriptSpeakerLine {
        // Pauses: count occurrences of "||"
        let pauses_count = raw_text.matches("||").count();

        // Clean pauses out
        let mut text = raw_text.replace("||", " ");

        // Cues: @{cue-name} or @{/cue-name}
        let cue_re = Regex::new(r"@\{/?([a-zA-Z0-9_-]+!?)\}").unwrap();
        let mut cues = Vec::new();
        for cap in cue_re.captures_iter(&text) {
            let full = cap.get(0).unwrap();
            let name = cap.get(1).unwrap().as_str().to_string();
            let is_closing = full.as_str().starts_with("@{/");
            cues.push(ScriptCue {
                name,
                is_closing,
                char_offset: full.start(),
            });
        }
        text = cue_re.replace_all(&text, "").to_string();

        // Phonetics: <original | spoken>
        let phon_re = Regex::new(r"<([^|>]+)\s*\|\s*([^>]+)>").unwrap();
        let mut phonetics = Vec::new();
        for cap in phon_re.captures_iter(&text) {
            let full = cap.get(0).unwrap();
            let orig = cap.get(1).unwrap().as_str().trim().to_string();
            let spoken = cap.get(2).unwrap().as_str().trim().to_string();
            phonetics.push(ScriptPhonetic {
                text: orig,
                phonetic_replacement: spoken,
                char_offset: full.start(),
            });
        }
        // In speech synthesis, we speak the replacement!
        let clean_speech_text = phon_re.replace_all(&text, "$2").trim().to_string();

        ScriptSpeakerLine {
            speaker: speaker.to_string(),
            raw_text: raw_text.to_string(),
            clean_speech_text,
            cues,
            phonetics,
            pauses_count,
        }
    }
}
