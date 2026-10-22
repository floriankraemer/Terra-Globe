use std::collections::BTreeMap;

use terra_core::{PlacemarkGeometry, Style};

use crate::ports::{EntityHandle, EntityLayer, LiveEditToken};

#[derive(Debug, Clone, PartialEq)]
pub struct FakeEntity {
    pub geometry: PlacemarkGeometry,
    pub style: Option<Style>,
    pub name: Option<String>,
    pub visible: bool,
    pub live_edit: bool,
}

/// Records every entity operation so tests can assert on the resulting scene.
#[derive(Debug, Default)]
pub struct FakeEntityLayer {
    pub entities: BTreeMap<String, FakeEntity>,
    pub removed: Vec<String>,
    next_id: u64,
}

impl FakeEntityLayer {
    pub fn get(&self, id: &str) -> Option<&FakeEntity> {
        self.entities.get(id)
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}

impl EntityLayer for FakeEntityLayer {
    fn create_entity(
        &mut self,
        geometry: &PlacemarkGeometry,
        id: Option<&str>,
        style: Option<&Style>,
        name: Option<&str>,
    ) -> EntityHandle {
        let entity_id = id.map_or_else(
            || {
                self.next_id += 1;
                format!("entity-{}", self.next_id)
            },
            str::to_owned,
        );
        self.entities.insert(
            entity_id.clone(),
            FakeEntity {
                geometry: geometry.clone(),
                style: style.cloned(),
                name: name.map(str::to_owned),
                visible: true,
                live_edit: false,
            },
        );
        EntityHandle { entity_id }
    }

    fn update_entity(
        &mut self,
        handle: &EntityHandle,
        geometry: &PlacemarkGeometry,
        style: Option<&Style>,
        name: Option<&str>,
    ) {
        if let Some(e) = self.entities.get_mut(&handle.entity_id) {
            e.geometry = geometry.clone();
            e.style = style.cloned();
            e.name = name.map(str::to_owned);
            e.live_edit = false;
        }
    }

    fn remove_entity(&mut self, handle: &EntityHandle) {
        if self.entities.remove(&handle.entity_id).is_some() {
            self.removed.push(handle.entity_id.clone());
        }
    }

    fn set_visible(&mut self, handle: &EntityHandle, visible: bool) {
        if let Some(e) = self.entities.get_mut(&handle.entity_id) {
            e.visible = visible;
        }
    }

    fn begin_live_geometry_edit(
        &mut self,
        handle: &EntityHandle,
        initial: &PlacemarkGeometry,
        style: Option<&Style>,
    ) -> LiveEditToken {
        if let Some(e) = self.entities.get_mut(&handle.entity_id) {
            e.geometry = initial.clone();
            e.style = style.cloned();
            e.live_edit = true;
        }
        LiveEditToken {
            entity_id: handle.entity_id.clone(),
        }
    }

    fn set_live_geometry(&mut self, token: &LiveEditToken, geometry: &PlacemarkGeometry) {
        if let Some(e) = self.entities.get_mut(&token.entity_id) {
            e.geometry = geometry.clone();
        }
    }
}
