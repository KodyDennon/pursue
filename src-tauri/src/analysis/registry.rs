use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefinition {
    pub id: String,
    pub name: String,
    pub model_type: ModelType,
    pub size_label: String,
    pub repo_id: String,
    pub filename: Option<String>, // Local filename for single-file models (ONNX/GGUF)
    pub repo_file: Option<String>,
    pub revision: String,
    pub expected_bytes: Option<u64>,
    pub expected_sha256: Option<String>,
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

impl ModelDefinition {
    pub fn download_url(&self) -> Option<String> {
        self.repo_file.as_ref().map(|repo_file| {
            format!(
                "https://huggingface.co/{}/resolve/{}/{}",
                self.repo_id, self.revision, repo_file
            )
        })
    }
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
            repo_file: Some("onnx/model.onnx".to_string()),
            revision: "01d3c3cd65ac9dc6bd0d702ed913366e7931097b".to_string(),
            expected_bytes: Some(133_093_490),
            expected_sha256: Some(
                "828e1496d7fabb79cfa4dcd84fa38625c0d3d21da474a00f08db0f559940cf35"
                    .to_string(),
            ),
            description: "High-performance semantic vector embedding engine.".to_string(),
        },
        ModelDefinition {
            id: "tokenizer".to_string(),
            name: "BGE Tokenizer".to_string(),
            model_type: ModelType::Tokenizer,
            size_label: "1 MB".to_string(),
            repo_id: "BAAI/bge-small-en-v1.5".to_string(),
            filename: Some("tokenizer.json".to_string()),
            repo_file: Some("tokenizer.json".to_string()),
            revision: "01d3c3cd65ac9dc6bd0d702ed913366e7931097b".to_string(),
            expected_bytes: Some(711_396),
            expected_sha256: Some(
                "d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66"
                    .to_string(),
            ),
            description: "Required for text-to-vector normalization.".to_string(),
        },
        ModelDefinition {
            id: "gemma-4-e4b-q4".to_string(),
            name: "Gemma 4 E4B IT (Official QAT Q4_0)".to_string(),
            model_type: ModelType::Intelligence,
            size_label: "5.15 GB".to_string(),
            repo_id: "google/gemma-4-E4B-it-qat-q4_0-gguf".to_string(),
            filename: Some("gemma-4-E4B_q4_0-it.gguf".to_string()),
            repo_file: Some("gemma-4-E4B_q4_0-it.gguf".to_string()),
            revision: "99ef3d9bbf819591699ffa9084c4be12db1fbe6c".to_string(),
            expected_bytes: Some(5_154_939_136),
            expected_sha256: Some(
                "e8b6a059ba86947a44ace84d6e5679795bc41862c25c30513142588f0e9dba1d"
                    .to_string(),
            ),
            description: "Google's official Gemma 4 E4B QAT Q4_0 checkpoint. The runtime offloads as much as possible to CUDA or Metal and uses CPU only after accelerator paths are unavailable."
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
        assert!(ids.contains(&"gemma-4-e4b-q4".to_string()));
    }

    #[test]
    fn test_registry_model_types() {
        let registry = get_model_registry();
        let embedding = registry.iter().find(|m| m.id == "bge-small").unwrap();
        let intelligence = registry.iter().find(|m| m.id == "gemma-4-e4b-q4").unwrap();

        assert_eq!(embedding.model_type, ModelType::Embedding);
        assert_eq!(intelligence.model_type, ModelType::Intelligence);
    }
}
