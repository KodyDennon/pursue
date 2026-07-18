use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefinition {
    pub id: String,
    pub name: String,
    pub model_type: ModelType,
    pub size_label: String,
    pub repo_id: String,
    pub filename: Option<String>, // For single file models (ONNX/GGUF)
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ModelType {
    Embedding,
    Tokenizer,
    Intelligence,
    Vision,
}

pub fn get_model_registry() -> Vec<ModelDefinition> {
    vec![
        ModelDefinition {
            id: "bge-small".to_string(),
            name: "BGE Small v1.5".to_string(),
            model_type: ModelType::Embedding,
            size_label: "134 MB".to_string(),
            repo_id: "BAAI/bge-small-en-v1.5".to_string(),
            filename: Some("bge-small-en-v1.5.onnx".to_string()),
            description: "High-performance semantic vector embedding engine.".to_string(),
        },
        ModelDefinition {
            id: "tokenizer".to_string(),
            name: "BGE Tokenizer".to_string(),
            model_type: ModelType::Tokenizer,
            size_label: "1 MB".to_string(),
            repo_id: "BAAI/bge-small-en-v1.5".to_string(),
            filename: Some("tokenizer.json".to_string()),
            description: "Required for text-to-vector normalization.".to_string(),
        },
        ModelDefinition {
            id: "gemma-3-1b-it".to_string(),
            name: "Gemma 3 1B IT".to_string(),
            model_type: ModelType::Intelligence,
            size_label: "2.1 GB".to_string(),
            repo_id: "google/gemma-3-1b-it".to_string(),
            filename: None,
            description: "Maintained Gemma 3 text model for reliable GPU-accelerated, evidence-grounded synthesis."
                .to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_contains_critical_models() {
        let registry = get_model_registry();
        let ids: Vec<String> = registry.iter().map(|m| m.id.clone()).collect();

        assert!(ids.contains(&"bge-small".to_string()));
        assert!(ids.contains(&"gemma-3-1b-it".to_string()));
    }

    #[test]
    fn test_registry_model_types() {
        let registry = get_model_registry();
        let embedding = registry.iter().find(|m| m.id == "bge-small").unwrap();
        let intelligence = registry.iter().find(|m| m.id == "gemma-3-1b-it").unwrap();

        assert_eq!(embedding.model_type, ModelType::Embedding);
        assert_eq!(intelligence.model_type, ModelType::Intelligence);
    }
}
