use regex::Regex;
use std::collections::BTreeMap;
use uuid::Uuid;
use crate::models::EntityHit;

/// Extracts known entity patterns from text.
pub fn extract_entities(text: &str) -> Vec<EntityHit> {
    let mut entities = BTreeMap::<(String, String), EntityHit>::new();

    let patterns = [
        (r"(?i)\b(AARO|NASA|DOJ|FBI|CIA|DHS|FAA|NORAD)\b", "agency"),
        (
            r"(?i)\b(orb|sphere|tic-tac|cylinder|disc|triangle)\b",
            "shape",
        ),
        (r"(?i)\b(radar|ir|flir|sonar|visual|satellite)\b", "sensor"),
        (
            r"(?i)\b(hypersonic|transmedium|instantaneous acceleration)\b",
            "pattern",
        ),
    ];

    for (pat, ty) in patterns {
        if let Ok(re) = Regex::new(pat) {
            for mat in re.find_iter(text) {
                add_entity(&mut entities, mat.as_str(), ty, 0.85, "deterministic");
            }
        }
    }

    let date_re = Regex::new(r"\b\d{4}-\d{2}-\d{2}\b").unwrap();
    for h in date_re.find_iter(text) {
        add_entity(&mut entities, h.as_str(), "date", 0.8, "deterministic");
    }

    entities.into_values().collect()
}

fn add_entity(e: &mut BTreeMap<(String, String), EntityHit>, n: &str, ty: &str, c: f64, s: &str) {
    let name = n.trim().to_string();
    if name.is_empty() {
        return;
    }
    e.entry((name.to_lowercase(), ty.to_string()))
        .or_insert(EntityHit {
            id: Uuid::new_v4().to_string(),
            name,
            entity_type: ty.to_string(),
            confidence: c,
            source: s.to_string(),
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_agencies() {
        let text = "The AARO and NASA are investigating. The FBI is not.";
        let entities = extract_entities(text);
        let names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
        assert!(names.contains(&"AARO".to_string()));
        assert!(names.contains(&"NASA".to_string()));
        assert!(names.contains(&"FBI".to_string()));
    }

    #[test]
    fn test_extract_shapes() {
        let text = "Observed a tic-tac shaped object and a sphere.";
        let entities = extract_entities(text);
        let types: Vec<String> = entities.iter().map(|e| e.entity_type.clone()).collect();
        assert_eq!(types.iter().filter(|t| *t == "shape").count(), 2);
    }

    #[test]
    fn test_extract_dates() {
        let text = "Incident occurred on 2024-05-12.";
        let entities = extract_entities(text);
        assert!(entities.iter().any(|e| e.name == "2024-05-12" && e.entity_type == "date"));
    }
}
