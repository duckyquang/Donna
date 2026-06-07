//! Keyword search over the knowledge graph for prompt injection.

use crate::error::Result;
use crate::knowledge::{self, KbNode};

const TOP_K: usize = 5;

/// Search knowledge nodes by keyword overlap; return a formatted block for the system prompt.
pub fn search_for_prompt(query: &str) -> Result<String> {
    let terms: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(String::from)
        .collect();

    if terms.is_empty() {
        return Ok(String::new());
    }

    let graph = knowledge::graph()?;
    let mut scored: Vec<(i32, &KbNode)> = graph
        .nodes
        .iter()
        .map(|node| (score_node(node, &terms), node))
        .filter(|(score, _)| *score > 0)
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.label.cmp(&b.1.label)));

    if scored.is_empty() {
        return Ok(String::new());
    }

    let mut lines = vec!["Relevant memories (keyword match):".to_string()];
    for (_, node) in scored.into_iter().take(TOP_K) {
        let path = node.folder.join(" / ");
        lines.push(format!(
            "- **{}** ({path}) — {}",
            node.label,
            truncate(&node.note, 200)
        ));
    }
    Ok(lines.join("\n"))
}

fn score_node(node: &KbNode, terms: &[String]) -> i32 {
    let haystack = format!(
        "{} {} {}",
        node.label.to_lowercase(),
        node.note.to_lowercase(),
        node.folder.join(" ").to_lowercase()
    );
    terms
        .iter()
        .map(|term| haystack.matches(term.as_str()).count() as i32)
        .sum()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let end = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    format!("{}…", &s[..end])
}
