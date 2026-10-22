use terra_core::{PlacemarkGeometry, Style};

/// Opaque handle to a rendered entity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityHandle {
    pub entity_id: String,
}

/// Token for an in-progress live geometry edit (drag preview).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LiveEditToken {
    pub entity_id: String,
}

/// Seam between scene logic and the concrete renderer (the `IEntityFactory`
/// of the TypeScript app). Keeps controllers and the synchronizer testable
/// without a GPU.
pub trait EntityLayer {
    fn create_entity(
        &mut self,
        geometry: &PlacemarkGeometry,
        id: Option<&str>,
        style: Option<&Style>,
        name: Option<&str>,
    ) -> EntityHandle;
    fn update_entity(
        &mut self,
        handle: &EntityHandle,
        geometry: &PlacemarkGeometry,
        style: Option<&Style>,
        name: Option<&str>,
    );
    fn remove_entity(&mut self, handle: &EntityHandle);
    fn set_visible(&mut self, handle: &EntityHandle, visible: bool);
    /// Switches an entity to cheap per-frame geometry updates (drag preview).
    /// Callers finish with a normal `update_entity` to restore static graphics.
    fn begin_live_geometry_edit(
        &mut self,
        handle: &EntityHandle,
        initial: &PlacemarkGeometry,
        style: Option<&Style>,
    ) -> LiveEditToken;
    fn set_live_geometry(&mut self, token: &LiveEditToken, geometry: &PlacemarkGeometry);
}
