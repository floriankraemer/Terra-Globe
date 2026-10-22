use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub visibility: bool,
    pub order: f64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct NewFolder {
    pub name: String,
    pub parent_id: Option<String>,
    pub order: f64,
    pub visibility: Option<bool>,
    pub description: Option<String>,
    pub open: Option<bool>,
}

impl NewFolder {
    pub fn new(name: impl Into<String>, parent_id: Option<String>, order: f64) -> Self {
        Self {
            name: name.into(),
            parent_id,
            order,
            ..Self::default()
        }
    }
}

/// Partial update (`Partial<Omit<Folder, "id">>`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FolderPatch {
    pub parent_id: Option<Option<String>>,
    pub name: Option<String>,
    pub visibility: Option<bool>,
    pub order: Option<f64>,
    pub updated_at: Option<String>,
    pub description: Option<Option<String>>,
    pub open: Option<Option<bool>>,
}

impl Folder {
    pub fn apply_patch(&mut self, patch: FolderPatch) {
        macro_rules! set {
            ($($field:ident),*) => { $( if let Some(v) = patch.$field { self.$field = v; } )* };
        }
        set!(
            parent_id,
            name,
            visibility,
            order,
            updated_at,
            description,
            open
        );
    }
}
