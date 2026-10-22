/// Small key/value settings store (the replacement for `localStorage`).
pub trait PreferencesStore: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, value: &str);
    fn remove(&self, key: &str);
}

/// Keys shared with the TypeScript app so settings survive the port.
pub mod keys {
    pub const SETTINGS: &str = "terra-globe:settings";
    pub const CAMERA_VIEW: &str = "terra-globe:cameraView";
    pub const SIDEBAR_WIDTH: &str = "terra-globe:sidebarWidth";
    pub const PLACEMARK_EDITOR_GEOMETRY: &str = "terra-globe:placemarkEditorGeometry";
    pub const HEIGHT_PROFILE_GEOMETRY: &str = "terra-globe:heightProfilePanelGeometry";
    pub const RULER_PANEL_GEOMETRY: &str = "terra-globe:rulerPanelGeometry";
    pub const ROUTE_PLANNER_GEOMETRY: &str = "terra-globe:routePlannerPanelGeometry";
}
