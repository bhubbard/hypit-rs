use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Default for SourceRange {
    fn default() -> Self {
        Self {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportKind {
    Module,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SvmlImport {
    pub kind: ImportKind,
    pub as_alias: Option<String>,
    pub from: Option<String>,
    pub source: Option<String>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum AttributeValue {
    Literal(String),
    Reference(String),
}

impl AttributeValue {
    pub fn as_str(&self) -> &str {
        match self {
            AttributeValue::Literal(s) => s.as_str(),
            AttributeValue::Reference(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SvmlNode {
    Element(SvmlElement),
    Text {
        value: String,
        range: SourceRange,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SvmlElement {
    pub name: String,
    pub attributes: BTreeMap<String, AttributeValue>,
    pub children: Vec<SvmlNode>,
    pub range: SourceRange,
}

impl SvmlElement {
    pub fn get_attr(&self, key: &str) -> Option<&AttributeValue> {
        self.attributes.get(key)
    }

    pub fn get_attr_str(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).map(|v| v.as_str())
    }

    pub fn text_content(&self) -> String {
        let mut s = String::new();
        for child in &self.children {
            match child {
                SvmlNode::Text { value, .. } => s.push_str(value),
                SvmlNode::Element(el) => s.push_str(&el.text_content()),
            }
        }
        s.trim().to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptCue {
    pub name: String,
    pub is_closing: bool,
    pub char_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptPhonetic {
    pub text: String,
    pub phonetic_replacement: String,
    pub char_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptSpeakerLine {
    pub speaker: String,
    pub raw_text: String,
    pub clean_speech_text: String,
    pub cues: Vec<ScriptCue>,
    pub phonetics: Vec<ScriptPhonetic>,
    pub pauses_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptSection {
    pub name: String,
    pub lines: Vec<ScriptSpeakerLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SvmlDocument {
    pub using: Option<String>,
    pub imports: Vec<SvmlImport>,
    pub scripts: Vec<ScriptSection>,
    pub text_values: BTreeMap<String, String>,
    pub root_elements: Vec<SvmlElement>,
}
