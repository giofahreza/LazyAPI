use std::path::PathBuf;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub name: String,
    pub values: IndexMap<String, String>,
    pub path: PathBuf,
    // In-flight scripts may still hold this environment after it is deleted.
    #[serde(skip)]
    pub deleted: bool,
}
