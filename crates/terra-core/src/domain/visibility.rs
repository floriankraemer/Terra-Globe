//! Visibility inheritance, mirroring `packages/core/src/domain/visibility.ts`.

use std::collections::HashMap;

use super::folder::Folder;
use super::placemark::Placemark;

/// A folder is visible only when it and every ancestor are visible.
pub fn is_folder_visible(folders: &[Folder], folder_id: Option<&str>) -> bool {
    let by_id: HashMap<&str, &Folder> = folders.iter().map(|f| (f.id.as_str(), f)).collect();
    let mut current = folder_id;
    // A malformed cycle must not hang the UI. Visiting each folder at most once
    // is enough: the first repeat cannot change the answer.
    let mut visited = 0usize;
    while let Some(id) = current {
        if visited > folders.len() {
            break;
        }
        visited += 1;
        let Some(folder) = by_id.get(id) else {
            break;
        };
        if !folder.visibility {
            return false;
        }
        current = folder.parent_id.as_deref();
    }
    true
}

pub fn is_placemark_visible(folders: &[Folder], placemark: &Placemark) -> bool {
    placemark.visibility && is_folder_visible(folders, placemark.folder_id.as_deref())
}

/// Resolves the visibility a renderer should use for a placemark.
pub fn effective_visibility(folders: &[Folder], placemark: &Placemark) -> bool {
    is_placemark_visible(folders, placemark)
}
