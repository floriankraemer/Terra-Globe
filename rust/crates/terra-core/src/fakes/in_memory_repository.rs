use std::collections::BTreeMap;
use std::sync::Arc;

use crate::domain::{
    Folder, FolderPatch, NewFolder, NewPlacemark, NewScreenOverlay, Placemark, PlacemarkPatch,
    ScreenOverlay, Style,
};
use crate::fakes::{FakeClock, SequentialIdGenerator};
use crate::ports::{
    Clock, DeleteFolderOptions, EntityKind, IdGenerator, ImportBatchPayload, PlacesRepository,
    RepositoryError, RepositoryResult,
};

/// Zero-I/O repository used by tests and as the reference implementation of
/// the storage contract.
pub struct InMemoryPlacesRepository {
    folders: BTreeMap<String, Folder>,
    placemarks: BTreeMap<String, Placemark>,
    styles: BTreeMap<String, Style>,
    screen_overlays: BTreeMap<String, ScreenOverlay>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGenerator>,
}

impl std::fmt::Debug for InMemoryPlacesRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryPlacesRepository")
            .field("folders", &self.folders.len())
            .field("placemarks", &self.placemarks.len())
            .field("styles", &self.styles.len())
            .field("screen_overlays", &self.screen_overlays.len())
            .finish_non_exhaustive()
    }
}

impl Default for InMemoryPlacesRepository {
    fn default() -> Self {
        Self::new(
            Arc::new(FakeClock::default()),
            Arc::new(SequentialIdGenerator::default()),
        )
    }
}

fn sorted_by_order<T: Clone>(items: impl Iterator<Item = T>, order: impl Fn(&T) -> f64) -> Vec<T> {
    let mut v: Vec<T> = items.collect();
    v.sort_by(|a, b| order(a).total_cmp(&order(b)));
    v
}

impl InMemoryPlacesRepository {
    pub fn new(clock: Arc<dyn Clock>, ids: Arc<dyn IdGenerator>) -> Self {
        Self {
            folders: BTreeMap::new(),
            placemarks: BTreeMap::new(),
            styles: BTreeMap::new(),
            screen_overlays: BTreeMap::new(),
            clock,
            ids,
        }
    }

    fn descendant_folder_ids(&self, root: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_owned()];
        while let Some(id) = stack.pop() {
            for f in self
                .folders
                .values()
                .filter(|f| f.parent_id.as_deref() == Some(&id))
            {
                stack.push(f.id.clone());
                out.push(f.id.clone());
            }
        }
        out
    }
}

impl PlacesRepository for InMemoryPlacesRepository {
    fn get_folder(&self, id: &str) -> RepositoryResult<Option<Folder>> {
        Ok(self.folders.get(id).cloned())
    }

    fn list_folders(&self, parent_id: Option<&str>) -> RepositoryResult<Vec<Folder>> {
        Ok(sorted_by_order(
            self.folders
                .values()
                .filter(|f| f.parent_id.as_deref() == parent_id)
                .cloned(),
            |f| f.order,
        ))
    }

    fn create_folder(&mut self, input: NewFolder) -> RepositoryResult<Folder> {
        if input.name.trim().is_empty() {
            return Err(RepositoryError::Validation("name must not be empty".into()));
        }
        let now = self.clock.now_iso8601();
        let folder = Folder {
            id: self.ids.next_id(),
            parent_id: input.parent_id,
            name: input.name,
            visibility: input.visibility.unwrap_or(true),
            order: input.order,
            created_at: now.clone(),
            updated_at: now,
            description: input.description,
            open: input.open,
        };
        self.folders.insert(folder.id.clone(), folder.clone());
        Ok(folder)
    }

    fn update_folder(&mut self, id: &str, patch: FolderPatch) -> RepositoryResult<Folder> {
        let folder = self
            .folders
            .get_mut(id)
            .ok_or_else(|| RepositoryError::NotFound {
                kind: EntityKind::Folder,
                id: id.into(),
            })?;
        folder.apply_patch(patch);
        Ok(folder.clone())
    }

    fn delete_folder(&mut self, id: &str, opts: DeleteFolderOptions) -> RepositoryResult<()> {
        if !self.folders.contains_key(id) {
            return Err(RepositoryError::NotFound {
                kind: EntityKind::Folder,
                id: id.into(),
            });
        }
        let has_children = self
            .folders
            .values()
            .any(|f| f.parent_id.as_deref() == Some(id))
            || self
                .placemarks
                .values()
                .any(|p| p.folder_id.as_deref() == Some(id))
            || self
                .screen_overlays
                .values()
                .any(|o| o.folder_id.as_deref() == Some(id));
        if has_children && !opts.recursive {
            return Err(RepositoryError::NotEmpty(id.into()));
        }
        let mut doomed = self.descendant_folder_ids(id);
        doomed.push(id.to_owned());
        self.placemarks.retain(|_, p| {
            !p.folder_id
                .as_deref()
                .is_some_and(|f| doomed.contains(&f.to_owned()))
        });
        self.screen_overlays.retain(|_, o| {
            !o.folder_id
                .as_deref()
                .is_some_and(|f| doomed.contains(&f.to_owned()))
        });
        self.folders.retain(|fid, _| !doomed.contains(fid));
        Ok(())
    }

    fn get_placemark(&self, id: &str) -> RepositoryResult<Option<Placemark>> {
        Ok(self.placemarks.get(id).cloned())
    }

    fn list_placemarks(&self, folder_id: Option<&str>) -> RepositoryResult<Vec<Placemark>> {
        Ok(sorted_by_order(
            self.placemarks
                .values()
                .filter(|p| p.folder_id.as_deref() == folder_id)
                .cloned(),
            |p| p.order,
        ))
    }

    fn create_placemark(&mut self, input: NewPlacemark) -> RepositoryResult<Placemark> {
        if input.name.trim().is_empty() {
            return Err(RepositoryError::Validation("name must not be empty".into()));
        }
        let geometry = input
            .geometry
            .ok_or_else(|| RepositoryError::Validation("geometry is required".into()))?;
        let now = self.clock.now_iso8601();
        let placemark = Placemark {
            id: self.ids.next_id(),
            folder_id: input.folder_id,
            name: input.name,
            description: input.description,
            geometry,
            style_id: input.style_id,
            visibility: input.visibility.unwrap_or(true),
            order: input.order.unwrap_or(0.0),
            created_at: now.clone(),
            updated_at: now,
            time_stamp: input.time_stamp,
            time_span_begin: input.time_span_begin,
            time_span_end: input.time_span_end,
            snippet: input.snippet,
            address: input.address,
            phone_number: input.phone_number,
            view: input.view,
            region: input.region,
            extended_data: input.extended_data,
            multi_geometry_group: input.multi_geometry_group,
        };
        self.placemarks
            .insert(placemark.id.clone(), placemark.clone());
        Ok(placemark)
    }

    fn update_placemark(&mut self, id: &str, patch: PlacemarkPatch) -> RepositoryResult<Placemark> {
        let placemark = self
            .placemarks
            .get_mut(id)
            .ok_or_else(|| RepositoryError::NotFound {
                kind: EntityKind::Placemark,
                id: id.into(),
            })?;
        placemark.apply_patch(patch);
        Ok(placemark.clone())
    }

    fn delete_placemark(&mut self, id: &str) -> RepositoryResult<()> {
        self.placemarks
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| RepositoryError::NotFound {
                kind: EntityKind::Placemark,
                id: id.into(),
            })
    }

    fn get_style(&self, id: &str) -> RepositoryResult<Option<Style>> {
        Ok(self.styles.get(id).cloned())
    }

    fn upsert_style(&mut self, style: Style) -> RepositoryResult<Style> {
        self.styles.insert(style.id.clone(), style.clone());
        Ok(style)
    }

    fn list_screen_overlays(
        &self,
        folder_id: Option<&str>,
    ) -> RepositoryResult<Vec<ScreenOverlay>> {
        Ok(sorted_by_order(
            self.screen_overlays
                .values()
                .filter(|o| o.folder_id.as_deref() == folder_id)
                .cloned(),
            |o| o.order,
        ))
    }

    fn create_screen_overlay(
        &mut self,
        input: NewScreenOverlay,
    ) -> RepositoryResult<ScreenOverlay> {
        if input.name.trim().is_empty() {
            return Err(RepositoryError::Validation("name must not be empty".into()));
        }
        if input.image_url.trim().is_empty() {
            return Err(RepositoryError::Validation(
                "imageUrl must not be empty".into(),
            ));
        }
        let now = self.clock.now_iso8601();
        let overlay = ScreenOverlay {
            id: self.ids.next_id(),
            folder_id: input.folder_id,
            name: input.name,
            image_url: input.image_url,
            overlay_xy: input.overlay_xy,
            screen_xy: input.screen_xy,
            size_x: input.size_x,
            size_y: input.size_y,
            rotation: input.rotation,
            visibility: input.visibility.unwrap_or(true),
            order: input.order.unwrap_or(0.0),
            created_at: now.clone(),
            updated_at: now,
        };
        self.screen_overlays
            .insert(overlay.id.clone(), overlay.clone());
        Ok(overlay)
    }

    fn delete_screen_overlay(&mut self, id: &str) -> RepositoryResult<()> {
        self.screen_overlays.remove(id);
        Ok(())
    }

    fn import_batch(&mut self, payload: ImportBatchPayload) -> RepositoryResult<()> {
        for f in payload.folders {
            self.folders.insert(f.id.clone(), f);
        }
        for p in payload.placemarks {
            self.placemarks.insert(p.id.clone(), p);
        }
        for s in payload.styles {
            self.styles.insert(s.id.clone(), s);
        }
        for o in payload.screen_overlays {
            self.screen_overlays.insert(o.id.clone(), o);
        }
        Ok(())
    }
}
