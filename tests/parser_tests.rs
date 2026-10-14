use hypit::compiler::SvmlCompiler;
use hypit::markup::SvmlParser;
use std::fs;
use std::path::Path;

#[test]
fn test_parse_interview_reference() {
    let path = Path::new("examples/interview/reference.svml");
    assert!(path.exists(), "reference.svml should exist");

    let content = fs::read_to_string(path).expect("read file");
    let parser = SvmlParser::new(&content);
    let doc = parser.parse().expect("parse svml");

    assert_eq!(doc.using, Some("@hypit/markup@1".to_string()));
    assert!(!doc.imports.is_empty(), "imports should not be empty");
    assert!(!doc.scripts.is_empty(), "scripts should not be empty");

    // Check script sections
    let section_names: Vec<&str> = doc.scripts.iter().map(|s| s.name.as_str()).collect();
    assert!(section_names.contains(&"manifest-rule"));
    assert!(section_names.contains(&"real-estate-rule"));
    assert!(section_names.contains(&"bitcoin-rule"));

    // Check compilation
    let plan = SvmlCompiler::compile(&doc, Some("Interview"));
    assert!(!plan.tasks.is_empty(), "plan tasks should not be empty");
    assert!(plan.speakers.contains(&"BOY".to_string()));
    assert!(plan.speakers.contains(&"WIFE".to_string()));
}

#[test]
fn test_parse_ranking_football() {
    let path = Path::new("examples/ranking-football/reference.svml");
    assert!(path.exists(), "ranking-football reference.svml should exist");

    let content = fs::read_to_string(path).expect("read file");
    let parser = SvmlParser::new(&content);
    let doc = parser.parse().expect("parse svml");

    assert!(!doc.scripts.is_empty());
    let plan = SvmlCompiler::compile(&doc, Some("Football Ranking"));
    assert!(plan.lines_count > 0);
    assert!(plan.speakers.contains(&"HOST".to_string()));
}
