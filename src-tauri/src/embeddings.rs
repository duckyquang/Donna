//! Ollama embedding index for knowledge-graph semantic retrieval.

use crate::db::Db;
use crate::error::Result;
use crate::knowledge::{self, KbNode};
use crate::providers;

pub const DEFAULT_EMBED_MODEL: &str = "nomic-embed-text";

pub fn node_key(node: &KbNode) -> String {
    format!("{}/{}", node.folder.join("/"), node.id)
}

fn node_text(node: &KbNode) -> String {
    format!(
        "{} {} {}",
        node.label,
        node.note,
        node.folder.join(" ")
    )
}

pub async fn index_node(db: &Db, host: &str, model: &str, node: &KbNode) -> Result<()> {
    let vector = providers::embed_ollama(host, model, &node_text(node)).await?;
    db.upsert_embedding(&node_key(node), &vector)
}

pub async fn reindex_all(db: &Db, host: &str, model: &str) -> Result<usize> {
    let graph = knowledge::graph()?;
    let mut count = 0usize;
    for node in &graph.nodes {
        if index_node(db, host, model, node).await.is_ok() {
            count += 1;
        }
    }
    Ok(count)
}

pub fn spawn_reindex(db: Db, host: String, model: String) {
    tauri::async_runtime::spawn(async move {
        let _ = reindex_all(&db, &host, &model).await;
    });
}
