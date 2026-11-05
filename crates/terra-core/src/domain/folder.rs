//! Folder entity, mirroring `packages/core/src/domain/folder.ts`.

use serde::{Deserialize, Serialize};

use super::DomainError;
use super::Patch;
use super::ids::new_id;
use super::time::now_iso8601;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub visibility: bool,
    pub order: i64,
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
    pub order: i64,
    pub visibility: Option<bool>,
    pub description: Option<String>,
    pub open: Option<bool>,
}

pub fn create_folder(input: NewFolder) -> Result<Folder, DomainError> {
    if input.name.trim().is_empty() {
        return Err(DomainError::EmptyName);
    }
    let now = now_iso8601();
    Ok(Folder {
        id: new_id(),
        parent_id: input.parent_id,
        name: input.name,
        visibility: input.visibility.unwrap_or(true),
        order: input.order,
        created_at: now.clone(),
        updated_at: now,
        description: input.description,
        open: input.open,
    })
}

/// Field-level update, where unset fields are left untouched and `Set(None)`
/// clears an optional field.
#[derive(Debug, Clone, Default)]
pub struct FolderPatch {
    pub name: Patch<String>,
    pub parent_id: Patch<Option<String>>,
    pub visibility: Patch<bool>,
    pub order: Patch<i64>,
    pub description: Patch<Option<String>>,
    pub open: Patch<Option<bool>>,
}

impl FolderPatch {
    pub fn apply(&self, folder: &mut Folder) {
        self.name.apply(&mut folder.name);
        self.parent_id.apply(&mut folder.parent_id);
        self.visibility.apply(&mut folder.visibility);
        self.order.apply(&mut folder.order);
        self.description.apply(&mut folder.description);
        self.open.apply(&mut folder.open);
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_unset()
            && self.parent_id.is_unset()
            && self.visibility.is_unset()
            && self.order.is_unset()
            && self.description.is_unset()
            && self.open.is_unset()
    }
}
