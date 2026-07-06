//! Hybrid keyword + embedding search over the knowledge graph for prompt injection.

use std::collections::HashMap;

use crate::db::Db;
use crate::embeddings;
use crate::error::Result;
use crate::knowledge::{self, KbNode};
use crate::providers;

const TOP_K: usize = 5;

pub struct RetrievalConfig<'a> {
    pub provider: &'a str,
    pub ollama_host: &'a str,
    pub embed_model: &'a str,
}

/// Search knowledge nodes by keyword overlap and optional Ollama embeddings.
pub async fn search_for_prompt(
    query: &str,
    db: &Db,
    config: &RetrievalConfig<'_>,
) -> Result<String> {
    let terms = tokenize(query);
    if terms.is_empty() {
        return Ok(String::new());
    }

    let graph = knowledge::graph()?;
    let mut scores: HashMap<String, f32> = HashMap::new();

    for node in &graph.nodes {
        let key = embeddings::node_key(node);
        let kw = keyword_score(node, &terms);
        if kw > 0 {
            scores.insert(key, kw as f32);
        }
    }

    if config.provider == "ollama" && !config.embed_model.is_empty() {
        if let Ok(query_vec) =
            providers::embed_ollama(config.ollama_host, config.embed_model, query).await
        {
            if !query_vec.is_empty() {
                for (key, vector) in db.list_embeddings()? {
                    let sim = cosine_similarity(&query_vec, &vector);
                    if sim > 0.3 {
                        scores
                            .entry(key)
                            .and_modify(|s| *s += sim * 10.0)
                            .or_insert(sim * 10.0);
                    }
                }
            }
        }
    }

    if scores.is_empty() {
        return Ok(String::new());
    }

    let node_by_key: HashMap<String, &KbNode> = graph
        .nodes
        .iter()
        .map(|n| (embeddings::node_key(n), n))
        .collect();

    let mut ranked: Vec<(&KbNode, f32)> = scores
        .into_iter()
        .filter_map(|(key, score)| node_by_key.get(&key).map(|n| (*n, score)))
        .collect();
    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.label.cmp(&b.0.label))
    });

    let method = if config.provider == "ollama" && !config.embed_model.is_empty() {
        "hybrid keyword + embedding match"
    } else {
        "keyword match"
    };
    let mut lines = vec![format!("Relevant memories ({method}):")];
    for (node, _) in ranked.into_iter().take(TOP_K) {
        let path = node.folder.join(" / ");
        lines.push(format!(
            "- **{}** ({path}) — {}",
            node.label,
            truncate(&node.note, 200)
        ));
    }
    Ok(lines.join("\n"))
}

fn tokenize(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(String::from)
        .collect()
}

fn keyword_score(node: &KbNode, terms: &[String]) -> i32 {
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

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let end = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    format!("{}…", &s[..end])
}
