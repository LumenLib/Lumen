use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Attachment {
    pub id: String,
    pub literature_id: String,
    pub file_path: String,
    pub file_name: String,
    pub file_size: u64,
    pub mime_type: Option<String>,
    pub etag: Option<String>,
    pub hash: Option<String>,
    pub is_main: bool,
    pub is_dirty: bool,
    pub is_deleted: bool,
    pub version: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Attachment {
    pub fn compute_labels(attachments: &[Attachment]) -> std::collections::HashMap<String, String> {
        let mut ext_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for file in attachments {
            if !file.is_main {
                let ext = std::path::Path::new(&file.file_name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("FILE")
                    .to_uppercase();
                *ext_counts.entry(ext).or_insert(0) += 1;
            }
        }

        let mut ext_indices: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut file_labels: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for file in attachments {
            let ext = std::path::Path::new(&file.file_name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("FILE")
                .to_uppercase();
            if file.is_main {
                file_labels.insert(file.id.clone(), ext);
            } else {
                let total_count = *ext_counts.get(&ext).unwrap_or(&0);
                if total_count > 1 {
                    let idx = ext_indices.entry(ext.clone()).or_insert(0);
                    *idx += 1;
                    file_labels.insert(file.id.clone(), format!("{}{}", ext, idx));
                } else {
                    file_labels.insert(file.id.clone(), ext);
                }
            }
        }
        file_labels
    }
}
