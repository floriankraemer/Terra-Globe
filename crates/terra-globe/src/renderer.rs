//! The wgpu globe renderer.
//!
//! Renders to an offscreen `Rgba8Unorm` texture so that any UI toolkit can
//! display it (egui registers the texture; the renderer has no egui dependency).
//! Depth is reverse-Z with an infinite far plane and every transform is
//! eye-relative.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use glam::DVec3;
use web_time::Instant;
use wgpu::util::DeviceExt;

use crate::camera::Camera;
use crate::flat::{
    FlatCamera, SceneMode, WORLD_WIDTH_METERS, build_flat_tile_mesh, select_flat_tiles,
};
use crate::geo::TileId;
use crate::imagery::{
    FALLBACK_RGBA, FetchedTile, ImageOverlay, ImageRgba, MAX_OVERLAY_IMAGE_SIDE, TileCache,
    TileFetcher, TileOverlay, TileProvider, limit_image_size, mip_chain, premultiply_srgb,
};
use crate::primitives::{FillMesh, LineMesh, build_meshes};
use crate::scene::{ROUTE_COLOR, Scene, TRANSIENT_COLOR, transient_style};
use crate::terrain::{Terrain, elevation_tile_for};
use crate::tiles::{
    LonLatRect, SelectedTile, SelectionOptions, TileMesh, build_flat_rect_mesh, build_polar_cap,
    build_rect_mesh_on, build_tile_vertices_on, coarser_neighbours, corner_level,
    nearest_ready_texture, select_tiles_with_relief, terrain_subdivisions, tile_mesh_indices,
};
use terra_providers::TileSource;

pub const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Fraction of the eye distance that lines, fills and draped images over
/// terrain are pulled towards the eye (see `pull_to_eye` in scene.wgsl).
const TERRAIN_DEPTH_PULL: f32 = 0.003;
/// Meshes of resident tiles rebuilt per frame when new elevation arrives;
/// the rest keep their old shape (their skirts cover the seams) until their turn.
const MESH_REBUILDS_PER_FRAME: usize = 12;
/// Frames a tile texture or mesh may go undrawn before it is dropped, once
/// its cache is over budget.
const RESOURCE_IDLE_FRAMES: u64 = 240;
/// Frames between re-tessellations of the placemark scene while elevation
/// tiles keep arriving.
const SCENE_REBUILD_INTERVAL_FRAMES: u64 = 30;

/// 4x MSAA on native. WebGL2 has no multisampled textures, so the browser build
/// stays at 1x until the downlevel pass in M7.
#[cfg(not(target_arch = "wasm32"))]
const SAMPLE_COUNT: u32 = 4;
#[cfg(target_arch = "wasm32")]
const SAMPLE_COUNT: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum GlobeError {
    #[error("surface size is zero")]
    ZeroSized,
    #[error("could not read back the rendered texture: {0}")]
    Readback(String),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderStats {
    pub tiles_drawn: usize,
    pub scene_items: usize,
    pub textures_ready: usize,
    pub requests_in_flight: usize,
    pub requests_queued: usize,
    pub deepest_level: u8,
    pub meshes_resident: usize,
    /// Elevation tiles downloading or waiting to; 0 with terrain off.
    pub terrain_requests: usize,
    /// Elevation tiles resident.
    pub terrain_tiles: usize,
    /// Globe tile meshes built or rebuilt this frame.
    pub meshes_built: usize,
    /// CPU time spent preparing the frame (selection, meshes, uploads),
    /// before the draw calls are recorded.
    pub prepare_micros: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniform {
    view_projection: [[f32; 4]; 4],
    viewport: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TileUniform {
    translation: [f32; 4],
    uv_rect: [f32; 4],
    tint: [f32; 4],
    /// Terrain hillshade: xyz = direction towards the light (ECEF axes), w =
    /// strength (0 = unshaded).
    light: [f32; 4],
    /// Local up at the tile, ECEF axes.
    up: [f32; 4],
}

struct GpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    center: DVec3,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bound_texture: Option<TileId>,
    /// Terrain the vertices were displaced by (see `terrain_stamp`); 0 = none.
    terrain_stamp: u64,
    /// Neighbour levels the edges were stitched to (see `coarser_neighbours`).
    stitched: [u8; 4],
    /// Levels the corners were sampled at (see `corner_level`).
    corners: [u8; 4],
    /// Grid cells per side; 0 for meshes that are not tile grids.
    subdivisions: u32,
    last_used_frame: u64,
}

struct GpuSceneBuffer {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

struct SceneDraw {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    bind_group: wgpu::BindGroup,
    fill: bool,
}

struct GpuSceneItem {
    fill: Option<GpuSceneBuffer>,
    line: Option<GpuSceneBuffer>,
    center: DVec3,
}

struct GpuTexture {
    view: wgpu::TextureView,
    last_used_frame: u64,
}

/// Per-draw uniform and bind group of an overlay. Overlays cannot share the
/// base mesh's own pair: they draw the same geometry with another texture and
/// opacity, and every draw in a pass needs its own uniform contents.
struct OverlayBinding {
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bound_texture: Option<TileId>,
}

/// One entry of the Layers panel: a transparent tile source with its own
/// fetch queue and texture cache.
struct TileLayer {
    overlay: TileOverlay,
    provider: TileProvider,
    fetcher: TileFetcher,
    textures: HashMap<TileId, GpuTexture>,
    bindings: HashMap<TileId, OverlayBinding>,
}

struct GpuGeometry {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    center: DVec3,
}

struct GpuImage {
    id: String,
    rect: LonLatRect,
    rotation_deg: f64,
    opacity: f32,
    /// Kept so the pointer used to detect an unchanged image cannot be
    /// recycled by the allocator for a different one.
    source: Arc<ImageRgba>,
    /// The second binding draws the copy shifted by one world width.
    bindings: [OverlayBinding; 2],
    globe_mesh: Option<GpuGeometry>,
    /// Terrain stamp `globe_mesh` was draped on.
    globe_stamp: u64,
    flat_mesh: Option<GpuGeometry>,
}

type Draw = (wgpu::Buffer, wgpu::Buffer, u32, wgpu::BindGroup);

pub struct GlobeRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: (u32, u32),
    color_texture: wgpu::Texture,
    color_view: wgpu::TextureView,
    msaa_view: Option<wgpu::TextureView>,
    depth_view: wgpu::TextureView,
    frame_buffer: wgpu::Buffer,
    frame_bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    tile_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    fallback_view: wgpu::TextureView,
    meshes: HashMap<TileId, GpuMesh>,
    /// Triangles of the globe tile grids by cells per side. Every tile of one
    /// resolution shares them.
    grid_indices: HashMap<u32, (wgpu::Buffer, u32)>,
    flat_meshes: HashMap<TileId, GpuMesh>,
    mode: SceneMode,
    textures: HashMap<TileId, GpuTexture>,
    /// Web-Mercator tiles stop at +/-85 degrees, so the poles get their own
    /// meshes and are drawn with the fallback colour.
    north_cap: GpuMesh,
    south_cap: GpuMesh,
    frame_index: u64,
    /// Bumped whenever the offscreen texture is recreated, so the UI knows to
    /// re-register the handle.
    generation: u64,
    options: SelectionOptions,
    stats: RenderStats,
    fill_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    scene_layout: wgpu::BindGroupLayout,
    scene: Scene,
    scene_revision: u64,
    scene_dirty: bool,
    scene_items: Vec<GpuSceneItem>,
    selection: Vec<crate::tiles::SelectedTile>,
    overlay_pipeline: wgpu::RenderPipeline,
    tile_layers: Vec<TileLayer>,
    images: Vec<GpuImage>,
    image_uploads: u64,
    tile_cache: Option<Arc<dyn TileCache>>,
    terrain: Terrain,
    /// Terrain stamp the scene meshes were last built for, and the frame that
    /// happened on, so arriving elevation tiles do not re-tessellate every frame.
    scene_stamp: u64,
    scene_built_frame: u64,
    meshes_built: usize,
}

impl GlobeRenderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terra-globe frame uniform"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terra-globe frame layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let frame_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terra-globe frame bind group"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });

        let tile_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terra-globe tile layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terra-globe shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/globe.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terra-globe pipeline layout"),
            bind_group_layouts: &[Some(&frame_layout), Some(&tile_layout)],
            immediate_size: 0,
        });

        let pipeline = tile_pipeline(&device, &pipeline_layout, &shader, TilePass::Base);
        let overlay_pipeline = tile_pipeline(&device, &pipeline_layout, &shader, TilePass::Overlay);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terra-globe tile sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            // Keeps the imagery sharp at grazing angles instead of letting the
            // mip level collapse to the blurriest one at the horizon.
            anisotropy_clamp: if cfg!(target_arch = "wasm32") { 1 } else { 8 },
            ..Default::default()
        });

        let fallback_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terra-globe fallback texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            fallback_texture.as_image_copy(),
            &FALLBACK_RGBA,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terra-globe scene item layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let scene_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terra-globe scene shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/scene.wgsl").into()),
        });

        let scene_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terra-globe scene pipeline layout"),
                bind_group_layouts: &[Some(&frame_layout), Some(&scene_layout)],
                immediate_size: 0,
            });

        let scene_primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        };
        let scene_depth = wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            // Transparent scene geometry must not hide what is behind it, but it
            // does need to depth-test against the globe.
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Greater),
            stencil: wgpu::StencilState::default(),
            // Pulls fills and strokes in front of the imagery they sit on,
            // which is otherwise exactly coplanar with the ellipsoid.
            bias: wgpu::DepthBiasState {
                constant: 4,
                slope_scale: 1.0,
                clamp: 0.0,
            },
        };
        let scene_blend = Some(wgpu::BlendState::ALPHA_BLENDING);

        let fill_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
        let fill_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terra-globe fill pipeline"),
            layout: Some(&scene_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &scene_shader,
                entry_point: Some("vs_fill"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<crate::primitives::FillVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &fill_attributes,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &scene_shader,
                entry_point: Some("fs_scene"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: SURFACE_FORMAT,
                    blend: scene_blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: scene_primitive,
            depth_stencil: Some(scene_depth.clone()),
            multisample: wgpu::MultisampleState {
                count: SAMPLE_COUNT,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });

        let line_attributes = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32,
            3 => Float32,
            4 => Float32,
            5 => Float32x4
        ];
        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terra-globe line pipeline"),
            layout: Some(&scene_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &scene_shader,
                entry_point: Some("vs_line"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<crate::primitives::LineVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &line_attributes,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &scene_shader,
                entry_point: Some("fs_scene"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: SURFACE_FORMAT,
                    blend: scene_blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: scene_primitive,
            depth_stencil: Some(scene_depth),
            multisample: wgpu::MultisampleState {
                count: SAMPLE_COUNT,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });

        let fallback_view = fallback_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let north_cap = upload_mesh(
            &device,
            &tile_layout,
            &sampler,
            &fallback_view,
            &build_polar_cap(true, 96, 6),
        );
        let south_cap = upload_mesh(
            &device,
            &tile_layout,
            &sampler,
            &fallback_view,
            &build_polar_cap(false, 96, 6),
        );

        let (color_texture, color_view, msaa_view, depth_view) =
            create_targets(&device, 1, 1, SAMPLE_COUNT);

        Self {
            device,
            queue,
            size: (1, 1),
            color_texture,
            color_view,
            msaa_view,
            depth_view,
            frame_buffer,
            frame_bind_group,
            pipeline,
            tile_layout,
            sampler,
            fallback_view,
            meshes: HashMap::new(),
            grid_indices: HashMap::new(),
            flat_meshes: HashMap::new(),
            mode: SceneMode::Scene3D,
            textures: HashMap::new(),
            north_cap,
            south_cap,
            frame_index: 0,
            generation: 0,
            options: SelectionOptions::default(),
            stats: RenderStats::default(),
            fill_pipeline,
            line_pipeline,
            scene_layout,
            scene: Scene::default(),
            scene_revision: u64::MAX,
            scene_dirty: false,
            scene_items: Vec::new(),
            selection: Vec::new(),
            overlay_pipeline,
            tile_layers: Vec::new(),
            images: Vec::new(),
            image_uploads: 0,
            tile_cache: None,
            terrain: Terrain::new(TileSource::terrain_terrarium()),
            scene_stamp: 0,
            scene_built_frame: 0,
            meshes_built: 0,
        }
    }

    pub fn terrain(&self) -> &Terrain {
        &self.terrain
    }

    pub fn terrain_mut(&mut self) -> &mut Terrain {
        &mut self.terrain
    }

    /// Identifies the terrain the 3D geometry has to be built for: 0 when it is
    /// flat (terrain off, or a flat scene mode), otherwise the generation of
    /// the elevation data. Anything built for another stamp is stale.
    fn terrain_stamp(&self, mode: SceneMode) -> u64 {
        if self.terrain.enabled() && !mode.is_flat() {
            self.terrain.generation() + 1
        } else {
            0
        }
    }

    pub fn device_handle(&self) -> wgpu::Device {
        self.device.clone()
    }

    pub fn queue_handle(&self) -> wgpu::Queue {
        self.queue.clone()
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.color_view
    }

    pub fn stats(&self) -> RenderStats {
        self.stats
    }

    pub fn set_selection_options(&mut self, options: SelectionOptions) {
        self.options = options;
    }

    /// Replaces the overlay scene. Meshes are rebuilt only when `revision`
    /// changes, so dragging the camera does not re-tessellate anything.
    pub fn set_scene(&mut self, revision: u64, scene: &Scene, mode: SceneMode) {
        if revision == self.scene_revision && mode == self.mode {
            return;
        }
        self.scene_revision = revision;
        self.mode = mode;
        self.scene = scene.clone();
        self.scene_dirty = true;
    }

    pub fn scene_item_count(&self) -> usize {
        self.scene_items.len()
    }

    /// Tiles drawn in the last frame, for diagnostics and LOD tests.
    pub fn last_selection(&self) -> &[crate::tiles::SelectedTile] {
        &self.selection
    }

    /// Neighbour levels the resident mesh of `id` has its edges stitched to
    /// (see `coarser_neighbours`), for tests and diagnostics.
    pub fn mesh_stitching(&self, id: TileId) -> Option<[u8; 4]> {
        self.meshes.get(&id).map(|mesh| mesh.stitched)
    }

    /// Zoom levels of every resident tile texture, for tests and diagnostics.
    pub fn ready_texture_levels(&self) -> Vec<u8> {
        let mut levels: Vec<u8> = self.textures.keys().map(|id| id.z).collect();
        levels.sort_unstable();
        levels
    }

    fn upload_scene_buffer<T: bytemuck::Pod>(
        &self,
        vertices: &[T],
        indices: &[u32],
        center: DVec3,
    ) -> Option<GpuSceneBuffer> {
        if vertices.is_empty() || indices.is_empty() {
            return None;
        }
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("terra-globe scene vertices"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("terra-globe scene indices"),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        let uniform_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terra-globe scene uniform"),
            size: std::mem::size_of::<TileUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terra-globe scene bind group"),
            layout: &self.scene_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let _ = center;
        Some(GpuSceneBuffer {
            vertex_buffer,
            index_buffer,
            index_count: u32::try_from(indices.len()).unwrap_or(u32::MAX),
            uniform_buffer,
            bind_group,
        })
    }

    /// Rebuilds overlay meshes from the current scene. Called only when the
    /// scene revision changes.
    fn rebuild_scene(&mut self) {
        self.scene_dirty = false;
        self.scene_items.clear();

        let mut items: Vec<(terra_core::PlacemarkGeometry, terra_core::Style, bool)> = self
            .scene
            .entries
            .iter()
            .filter(|entry| entry.visible)
            .map(|entry| (entry.geometry.clone(), entry.style.clone(), entry.selected))
            .collect();

        let transient = transient_style(TRANSIENT_COLOR, 2.0);
        if let Some(preview) = &self.scene.preview {
            items.push((preview.clone(), transient.clone(), false));
        }
        if let Some(ruler) = &self.scene.ruler_path {
            if ruler.len() >= 2 {
                if let Ok(geometry) =
                    terra_core::PlacemarkGeometry::line_string(ruler.clone(), None, None)
                {
                    items.push((geometry, transient.clone(), false));
                }
            }
        }
        if let Some(route) = &self.scene.route_path {
            if route.len() >= 2 {
                if let Ok(geometry) =
                    terra_core::PlacemarkGeometry::line_string(route.clone(), None, None)
                {
                    items.push((geometry, transient_style(ROUTE_COLOR, 4.0), false));
                }
            }
        }
        if let Some(rings) = self.scene.rings {
            let ring_style = terra_core::Style {
                outline_enabled: true,
                outline_color: "#ffffff".to_string(),
                outline_width: 1.0,
                filled: false,
                fill_color: "#ffffff".to_string(),
                fill_opacity: 0.08,
                ..terra_core::Style::default()
            };
            for index in 1..=rings.count {
                let radius = rings.spacing_meters * index as f64;
                if let Ok(geometry) = terra_core::PlacemarkGeometry::circle(rings.center, radius) {
                    items.push((geometry, ring_style.clone(), false));
                }
            }
        }

        for (geometry, style, selected) in items {
            let ground = |lon: f64, lat: f64| self.terrain.ground(lon, lat);
            let (fill, line) = build_meshes(&geometry, &style, selected, self.mode, &ground);
            // The mesh knows its own frame (ECEF or the flat plane); using the
            // geometry centre here would put flat overlays in the wrong world.
            let center = fill
                .as_ref()
                .map(|mesh: &FillMesh| mesh.center)
                .or_else(|| line.as_ref().map(|mesh: &LineMesh| mesh.center))
                .unwrap_or_default();
            let fill = fill.and_then(|mesh: FillMesh| {
                self.upload_scene_buffer(&mesh.vertices, &mesh.indices, mesh.center)
            });
            let line = line.and_then(|mesh: LineMesh| {
                self.upload_scene_buffer(&mesh.vertices, &mesh.indices, mesh.center)
            });
            if fill.is_some() || line.is_some() {
                self.scene_items.push(GpuSceneItem { fill, line, center });
            }
        }
    }

    fn prepare_scene_draws(&mut self, eye: DVec3, pull: f32) -> Vec<SceneDraw> {
        let mut draws = Vec::new();
        for item in &mut self.scene_items {
            let translation = (item.center - eye).as_vec3();
            for (buffer, fill) in [(&mut item.fill, true), (&mut item.line, false)] {
                let Some(buffer) = buffer else {
                    continue;
                };
                self.queue.write_buffer(
                    &buffer.uniform_buffer,
                    0,
                    bytemuck::bytes_of(&TileUniform {
                        translation: [translation.x, translation.y, translation.z, pull],
                        uv_rect: [0.0, 0.0, 1.0, 1.0],
                        tint: [1.0, 1.0, 1.0, 1.0],
                        light: [0.0; 4],
                        up: [0.0; 4],
                    }),
                );
                draws.push(SceneDraw {
                    vertex_buffer: buffer.vertex_buffer.clone(),
                    index_buffer: buffer.index_buffer.clone(),
                    index_count: buffer.index_count,
                    bind_group: buffer.bind_group.clone(),
                    fill,
                });
            }
        }
        draws
    }

    /// Drops cached meshes and imagery, e.g. after the base layer changes.
    /// Bumps the generation so the UI re-registers the surface handle.
    pub fn clear_caches(&mut self) {
        self.meshes.clear();
        self.textures.clear();
        self.generation += 1;
    }

    /// Recreates the offscreen colour and depth textures when the viewport
    /// changes. Bumps [`Self::generation`] so the UI re-registers the handle.
    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if (width, height) == self.size {
            return;
        }
        let (color_texture, color_view, msaa_view, depth_view) =
            create_targets(&self.device, width, height, SAMPLE_COUNT);
        self.color_texture = color_texture;
        self.color_view = color_view;
        self.msaa_view = msaa_view;
        self.depth_view = depth_view;
        self.size = (width, height);
        self.generation += 1;
    }

    /// Uploads decoded tiles and drops textures that have not been used for a
    /// while. Called from `render`, but public so the app can pump it while the
    /// window is not visible.
    pub fn ingest(&mut self, fetched: Vec<FetchedTile>) {
        for tile in fetched {
            let Ok(image) = tile.result else {
                continue;
            };
            let view = upload_srgb_texture(
                &self.device,
                &self.queue,
                &mip_chain(&image),
                "terra-globe tile texture",
            );
            self.textures.insert(
                tile.id,
                GpuTexture {
                    view,
                    // Frame 0 would read as "unused for ages" and get the tile
                    // evicted the moment the cache is over budget.
                    last_used_frame: self.frame_index,
                },
            );
        }
    }

    /// Replaces the stack of transparent tile layers, bottom to top.
    ///
    /// Layers are matched by source: one that is unchanged keeps its textures
    /// and queue (an opacity change costs nothing), a removed one drops its
    /// textures, a new one starts empty. A repeated source id is ignored.
    pub fn set_tile_overlays(&mut self, overlays: &[TileOverlay]) {
        let mut previous = std::mem::take(&mut self.tile_layers);
        let mut next: Vec<TileLayer> = Vec::with_capacity(overlays.len());
        for overlay in overlays {
            if next
                .iter()
                .any(|layer| layer.overlay.source.id == overlay.source.id)
            {
                continue;
            }
            let overlay = TileOverlay {
                source: overlay.source.clone(),
                opacity: sanitize_opacity(overlay.opacity),
            };
            let mut layer = match previous
                .iter()
                .position(|layer| layer.overlay.source == overlay.source)
            {
                Some(index) => previous.remove(index),
                None => TileLayer {
                    provider: TileProvider::new(overlay.source.clone()),
                    overlay: overlay.clone(),
                    fetcher: match &self.tile_cache {
                        Some(cache) => TileFetcher::new().with_cache(Arc::clone(cache)),
                        None => TileFetcher::new(),
                    },
                    textures: HashMap::new(),
                    bindings: HashMap::new(),
                },
            };
            layer.overlay.opacity = overlay.opacity;
            next.push(layer);
        }
        self.tile_layers = next;
    }

    /// Shares the persistent tile store with every overlay layer, present and
    /// future, the same way the basemap fetcher uses it.
    pub fn set_overlay_cache(&mut self, cache: Arc<dyn TileCache>) {
        for layer in &mut self.tile_layers {
            layer.fetcher.set_cache(Arc::clone(&cache));
        }
        self.tile_cache = Some(cache);
    }

    /// Ids of the active tile layers, bottom to top.
    pub fn tile_overlay_ids(&self) -> Vec<String> {
        self.tile_layers
            .iter()
            .map(|layer| layer.overlay.source.id.clone())
            .collect()
    }

    /// Hands decoded tiles to one overlay layer, as [`Self::ingest`] does for
    /// the basemap. Unknown layers are ignored.
    pub fn ingest_overlay(&mut self, layer_id: &str, fetched: Vec<FetchedTile>) {
        let frame = self.frame_index;
        let Some(layer) = self
            .tile_layers
            .iter_mut()
            .find(|layer| layer.overlay.source.id == layer_id)
        else {
            return;
        };
        ingest_layer(&self.device, &self.queue, layer, frame, fetched);
    }

    pub fn has_overlay_texture(&self, layer_id: &str, id: TileId) -> bool {
        self.tile_layers
            .iter()
            .find(|layer| layer.overlay.source.id == layer_id)
            .is_some_and(|layer| layer.textures.contains_key(&id))
    }

    /// Replaces the image overlays, drawn in the given order above the tile
    /// layers. Matched by id: an unchanged picture is not uploaded again, and a
    /// new box, rotation or opacity only rebuilds the cheap parts. Overlays with
    /// an empty image or an unusable box are skipped.
    pub fn set_image_overlays(&mut self, overlays: &[ImageOverlay]) {
        let mut previous = std::mem::take(&mut self.images);
        let mut next: Vec<GpuImage> = Vec::with_capacity(overlays.len());
        let max_side = self
            .device
            .limits()
            .max_texture_dimension_2d
            .min(MAX_OVERLAY_IMAGE_SIDE);
        for overlay in overlays {
            if next.iter().any(|image| image.id == overlay.id) {
                continue;
            }
            let Some(rect) =
                LonLatRect::new(overlay.west, overlay.south, overlay.east, overlay.north)
            else {
                tracing::warn!(id = %overlay.id, "image overlay has unusable bounds");
                continue;
            };
            if overlay.image.width() == 0 || overlay.image.height() == 0 {
                continue;
            }
            let rotation_deg = if overlay.rotation_deg.is_finite() {
                overlay.rotation_deg
            } else {
                0.0
            };
            let reusable = previous.iter().position(|image| {
                image.id == overlay.id && Arc::ptr_eq(&image.source, &overlay.image)
            });
            let mut image = match reusable {
                Some(index) => previous.remove(index),
                None => {
                    self.image_uploads += 1;
                    let limited = limit_image_size(premultiply_srgb(&overlay.image), max_side);
                    let view = upload_srgb_texture(
                        &self.device,
                        &self.queue,
                        &mip_chain(&limited),
                        "terra-globe image overlay",
                    );
                    GpuImage {
                        id: overlay.id.clone(),
                        rect,
                        rotation_deg,
                        opacity: 0.0,
                        source: Arc::clone(&overlay.image),
                        bindings: [0, 1].map(|_| {
                            create_overlay_binding(
                                &self.device,
                                &self.tile_layout,
                                &self.sampler,
                                &self.fallback_view,
                                Some(&view),
                            )
                        }),
                        globe_mesh: None,
                        globe_stamp: 0,
                        flat_mesh: None,
                    }
                }
            };
            if image.rect != rect || image.rotation_deg != rotation_deg {
                image.rect = rect;
                image.rotation_deg = rotation_deg;
                image.globe_mesh = None;
                image.flat_mesh = None;
            }
            image.opacity = sanitize_opacity(overlay.opacity);
            next.push(image);
        }
        self.images = next;
    }

    pub fn image_overlay_count(&self) -> usize {
        self.images.len()
    }

    /// How many image textures have been uploaded so far, for tests that check
    /// unchanged overlays are not uploaded again.
    pub fn image_upload_count(&self) -> u64 {
        self.image_uploads
    }

    pub fn has_texture(&self, id: TileId) -> bool {
        self.textures.contains_key(&id)
    }

    /// Renders one frame and returns statistics for the debug overlay.
    pub fn render(
        &mut self,
        mode: SceneMode,
        camera: &Camera,
        flat: &FlatCamera,
        provider: &TileProvider,
        fetcher: &mut TileFetcher,
    ) -> RenderStats {
        let started = Instant::now();
        self.frame_index += 1;
        self.meshes_built = 0;
        if mode != self.mode {
            self.mode = mode;
            self.scene_dirty = true;
        }
        let aspect = f64::from(self.size.0) / f64::from(self.size.1);
        let view_projection = if mode.is_flat() {
            flat.view_projection(aspect)
        } else {
            camera.view_projection(aspect)
        };
        self.queue.write_buffer(
            &self.frame_buffer,
            0,
            bytemuck::bytes_of(&FrameUniform {
                view_projection: view_projection.to_cols_array_2d(),
                viewport: [self.size.0 as f32, self.size.1 as f32, 0.0, 0.0],
            }),
        );

        self.ingest(fetcher.poll(provider));
        let stamp = self.terrain_stamp(mode);
        if stamp != 0 {
            self.terrain.poll();
        }

        let ready: HashSet<TileId> = self.textures.keys().copied().collect();
        let selection = if mode.is_flat() {
            select_flat_tiles(
                flat,
                self.size,
                provider.max_level(),
                self.options.max_tiles.max(60) * 2,
                &|id| ready.contains(&id),
            )
        } else {
            let terrain = &self.terrain;
            select_tiles_with_relief(
                camera,
                self.size,
                provider.max_level(),
                self.options,
                &|id| ready.contains(&id),
                &|id| {
                    (stamp != 0)
                        .then(|| terrain.cache().relief(elevation_tile_for(id)))
                        .flatten()
                },
            )
        };
        if stamp != 0 {
            self.terrain.request(selection.iter().map(|tile| tile.id));
        }

        let mut selected_ids = HashSet::with_capacity(selection.len());
        let mut rebuilds_left = MESH_REBUILDS_PER_FRAME;
        let leaves: HashSet<TileId> = selection.iter().map(|tile| tile.id).collect();
        for tile in &selection {
            selected_ids.insert(tile.id);
            let texture_key = tile.texture.map(|(id, _)| id);
            // Marked before eviction runs below, so nothing this frame draws
            // can be evicted out from under it.
            if let Some(texture) = texture_key.and_then(|key| self.textures.get_mut(&key)) {
                texture.last_used_frame = self.frame_index;
            }
            if mode.is_flat() {
                self.ensure_flat_mesh(tile.id);
                self.rebind_flat(tile.id, texture_key);
            } else {
                let (stitched, corners) = if stamp == 0 {
                    ([0; 4], [0; 4])
                } else {
                    let is_leaf = |id: TileId| leaves.contains(&id);
                    (
                        coarser_neighbours(tile.id, &is_leaf),
                        [0, 1, 2, 3].map(|corner| corner_level(tile.id, corner, &is_leaf)),
                    )
                };
                self.ensure_mesh(
                    tile.id,
                    stamp,
                    stitched,
                    corners,
                    &leaves,
                    &mut rebuilds_left,
                );
                self.rebind(tile.id, texture_key);
            }
        }

        // Request every missing tile in the selection, plus its ancestors so the
        // fallback chain fills in progressively. Gating this on "no ancestor is
        // ready" was the bug that stopped deeper imagery from ever loading.
        fetcher.set_desired(provider, desired_requests(&selection, &ready));

        // Keep a little history for parent fallback when the camera jitters,
        // but never let the caches grow without bound.
        // Tiles that left the view usually come back (the camera turns, or
        // zooms out again), so the meshes used longest ago go first, rather
        // than everything that is not on screen at once.
        let mesh_budget = self.options.max_tiles.saturating_mul(2);
        if self.meshes.len() > mesh_budget {
            let mut used: Vec<u64> = self
                .meshes
                .values()
                .map(|mesh| mesh.last_used_frame)
                .collect();
            used.sort_unstable_by(|a, b| b.cmp(a));
            let oldest_kept = used.get(mesh_budget - 1).copied().unwrap_or(0);
            self.meshes
                .retain(|_, mesh| mesh.last_used_frame >= oldest_kept);
        }
        if self.flat_meshes.len() > self.options.max_tiles.saturating_mul(4) {
            self.flat_meshes.retain(|id, _| selected_ids.contains(id));
        }
        if self.textures.len() > self.options.max_tiles.saturating_mul(2) {
            let mut evicted = Vec::new();
            self.textures.retain(|id, texture| {
                let keep =
                    self.frame_index.saturating_sub(texture.last_used_frame) < RESOURCE_IDLE_FRAMES;
                if !keep {
                    evicted.push(*id);
                }
                keep
            });
            if !evicted.is_empty() {
                // A mesh bound to an evicted texture keeps it alive through its
                // bind group and would never rebind, so drop those meshes; they
                // are a few hundred vertices and get rebuilt on demand.
                let evicted: HashSet<TileId> = evicted.into_iter().collect();
                let stale =
                    |mesh: &GpuMesh| mesh.bound_texture.is_some_and(|id| evicted.contains(&id));
                self.meshes.retain(|_, mesh| !stale(mesh));
                self.flat_meshes.retain(|_, mesh| !stale(mesh));
            }
        }

        let eye = if mode.is_flat() {
            flat.eye_plane()
        } else {
            camera.eye_ecef()
        };
        let mut draws: Vec<Draw> = Vec::with_capacity(selection.len());
        for tile in &selection {
            let mesh = if mode.is_flat() {
                self.flat_meshes.get_mut(&tile.id)
            } else {
                self.meshes.get_mut(&tile.id)
            };
            let Some(mesh) = mesh else {
                continue;
            };
            let translation = (mesh.center - eye).as_vec3();
            let uv_rect = tile.texture.map_or([0.0, 0.0, 1.0, 1.0], |(_, uv)| uv);
            write_tile_uniform(&self.queue, mesh, translation, uv_rect, stamp != 0);
            draws.push(draw_of(mesh));
        }
        // Counted before the polar caps are appended: they are not tiles.
        let tiles_drawn = draws.len();

        if !mode.is_flat() {
            for cap in [&mut self.north_cap, &mut self.south_cap] {
                let translation = (cap.center - eye).as_vec3();
                write_tile_uniform(&self.queue, cap, translation, [0.0, 0.0, 1.0, 1.0], false);
                draws.push(draw_of(cap));
            }
        }

        let overlay_draws = self.prepare_overlay_draws(mode, eye, &selection, stamp);

        let scene_stale = stamp != self.scene_stamp
            && (stamp == 0
                || self.scene_stamp == 0
                || self.frame_index.saturating_sub(self.scene_built_frame)
                    >= SCENE_REBUILD_INTERVAL_FRAMES);
        if self.scene_dirty || scene_stale {
            self.rebuild_scene();
            self.scene_stamp = stamp;
            self.scene_built_frame = self.frame_index;
        }
        let pull = if stamp == 0 { 0.0 } else { TERRAIN_DEPTH_PULL };
        let scene_draws = self.prepare_scene_draws(eye, pull);
        let prepare_micros = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("terra-globe frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terra-globe pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.msaa_view.as_ref().unwrap_or(&self.color_view),
                    resolve_target: self.msaa_view.as_ref().map(|_| &self.color_view),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.008,
                            g: 0.012,
                            b: 0.02,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            for (vertex_buffer, index_buffer, index_count, bind_group) in &draws {
                pass.set_bind_group(1, bind_group, &[]);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*index_count, 0, 0..1);
            }

            if !overlay_draws.is_empty() {
                pass.set_pipeline(&self.overlay_pipeline);
                for (vertex_buffer, index_buffer, index_count, bind_group) in &overlay_draws {
                    pass.set_bind_group(1, bind_group, &[]);
                    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..*index_count, 0, 0..1);
                }
            }

            pass.set_pipeline(&self.fill_pipeline);
            for draw in scene_draws.iter().filter(|draw| draw.fill) {
                pass.set_bind_group(1, &draw.bind_group, &[]);
                pass.set_vertex_buffer(0, draw.vertex_buffer.slice(..));
                pass.set_index_buffer(draw.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..draw.index_count, 0, 0..1);
            }

            pass.set_pipeline(&self.line_pipeline);
            for draw in scene_draws.iter().filter(|draw| !draw.fill) {
                pass.set_bind_group(1, &draw.bind_group, &[]);
                pass.set_vertex_buffer(0, draw.vertex_buffer.slice(..));
                pass.set_index_buffer(draw.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..draw.index_count, 0, 0..1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));

        self.selection = selection;

        self.stats = RenderStats {
            tiles_drawn,
            textures_ready: self.textures.len(),
            requests_in_flight: fetcher.in_flight_count()
                + self
                    .tile_layers
                    .iter()
                    .map(|layer| layer.fetcher.in_flight_count())
                    .sum::<usize>(),
            requests_queued: fetcher.queued_count()
                + self
                    .tile_layers
                    .iter()
                    .map(|layer| layer.fetcher.queued_count())
                    .sum::<usize>(),
            deepest_level: self
                .selection
                .iter()
                .map(|tile| tile.id.z)
                .max()
                .unwrap_or(0),
            meshes_resident: self.meshes.len(),
            scene_items: self.scene_items.len(),
            terrain_requests: self.terrain.in_flight() + self.terrain.queued(),
            terrain_tiles: self.terrain.cache().len(),
            meshes_built: self.meshes_built,
            prepare_micros,
        };
        self.stats
    }

    /// Draws for every tile layer, then every image overlay, in stacking order.
    fn prepare_overlay_draws(
        &mut self,
        mode: SceneMode,
        eye: DVec3,
        selection: &[SelectedTile],
        stamp: u64,
    ) -> Vec<Draw> {
        let mut draws = Vec::new();
        let mut layers = std::mem::take(&mut self.tile_layers);
        for layer in &mut layers {
            self.prepare_tile_layer(layer, mode, eye, selection, &mut draws);
        }
        self.tile_layers = layers;

        let mut images = std::mem::take(&mut self.images);
        for image in &mut images {
            self.prepare_image(image, mode, eye, stamp, &mut draws);
        }
        self.images = images;
        draws
    }

    fn prepare_tile_layer(
        &self,
        layer: &mut TileLayer,
        mode: SceneMode,
        eye: DVec3,
        selection: &[SelectedTile],
        draws: &mut Vec<Draw>,
    ) {
        let fetched = layer.fetcher.poll(&layer.provider);
        ingest_layer(&self.device, &self.queue, layer, self.frame_index, fetched);
        // A hidden layer neither draws nor downloads; its textures stay so
        // fading it back in is instant.
        if layer.overlay.opacity <= 0.0 {
            return;
        }

        let ready: HashSet<TileId> = layer.textures.keys().copied().collect();
        let is_ready = |id: TileId| ready.contains(&id);
        let max_level = layer.provider.max_level();
        let mut wanted = Vec::with_capacity(selection.len());
        for tile in selection {
            // The base may be deeper than this layer's imagery goes; ask for
            // the deepest tile the source has and stretch it.
            let mut target = tile.id;
            while target.z > max_level {
                let Some(parent) = target.parent() else {
                    break;
                };
                target = parent;
            }
            wanted.push(SelectedTile {
                id: target,
                texture: nearest_ready_texture(target, &is_ready),
            });
        }
        layer
            .fetcher
            .set_desired(&layer.provider, desired_requests(&wanted, &ready));

        for tile in selection {
            let Some((key, uv_rect)) = nearest_ready_texture(tile.id, &is_ready) else {
                continue;
            };
            let mesh = if mode.is_flat() {
                self.flat_meshes.get(&tile.id)
            } else {
                self.meshes.get(&tile.id)
            };
            let (Some(mesh), Some(texture)) = (mesh, layer.textures.get_mut(&key)) else {
                continue;
            };
            texture.last_used_frame = self.frame_index;
            let binding = layer.bindings.entry(tile.id).or_insert_with(|| {
                create_overlay_binding(
                    &self.device,
                    &self.tile_layout,
                    &self.sampler,
                    &self.fallback_view,
                    None,
                )
            });
            if binding.bound_texture != Some(key) {
                binding.bind_group = create_bind_group(
                    &self.device,
                    &self.tile_layout,
                    &self.sampler,
                    &self.fallback_view,
                    Some(&texture.view),
                    &binding.uniform_buffer,
                );
                binding.bound_texture = Some(key);
            }
            write_overlay_uniform(
                &self.queue,
                &binding.uniform_buffer,
                (mesh.center - eye).as_vec3(),
                uv_rect,
                layer.overlay.opacity,
                0.0,
            );
            draws.push((
                mesh.vertex_buffer.clone(),
                mesh.index_buffer.clone(),
                mesh.index_count,
                binding.bind_group.clone(),
            ));
        }

        let budget = self.options.max_tiles.saturating_mul(2);
        if layer.textures.len() > budget {
            layer.textures.retain(|_, texture| {
                self.frame_index.saturating_sub(texture.last_used_frame) < RESOURCE_IDLE_FRAMES
            });
        }
        // A binding keeps its texture alive, so drop those whose texture is
        // gone, and bound the rest like the base meshes.
        let selected: HashSet<TileId> = selection.iter().map(|tile| tile.id).collect();
        let over_budget = layer.bindings.len() > self.options.max_tiles.saturating_mul(4);
        let textures = &layer.textures;
        layer.bindings.retain(|id, binding| {
            binding
                .bound_texture
                .is_none_or(|key| textures.contains_key(&key))
                && (!over_budget || selected.contains(id))
        });
    }

    fn prepare_image(
        &self,
        image: &mut GpuImage,
        mode: SceneMode,
        eye: DVec3,
        stamp: u64,
        draws: &mut Vec<Draw>,
    ) {
        if image.opacity <= 0.0 {
            return;
        }
        let flat = mode.is_flat();
        let (rect, rotation) = (image.rect, image.rotation_deg);
        if !flat && image.globe_stamp != stamp {
            // Draped on other elevation (or none) than the mesh was built for.
            // Throttled like the scene: new tiles arrive in bursts.
            let throttled = image.globe_mesh.is_some()
                && stamp != 0
                && image.globe_stamp != 0
                && !self
                    .frame_index
                    .is_multiple_of(SCENE_REBUILD_INTERVAL_FRAMES);
            if !throttled {
                image.globe_mesh = None;
            }
        }
        let slot = if flat {
            &mut image.flat_mesh
        } else {
            &mut image.globe_mesh
        };
        let globe_stamp = &mut image.globe_stamp;
        let geometry = slot.get_or_insert_with(|| {
            let mesh = if flat {
                build_flat_rect_mesh(&rect, rotation)
            } else {
                *globe_stamp = stamp;
                let ground = |lon: f64, lat: f64| self.terrain.ground(lon, lat);
                build_rect_mesh_on(&rect, rotation, (stamp != 0).then_some(&ground))
            };
            upload_geometry(&self.device, &mesh)
        });
        let pull = if !flat && stamp != 0 {
            TERRAIN_DEPTH_PULL
        } else {
            0.0
        };

        // The flat map does not repeat, so a box across the antimeridian is
        // drawn twice, one world apart, to appear on both edges.
        let shifts: &[f64] = if flat && rect.crosses_antimeridian() {
            &[0.0, -WORLD_WIDTH_METERS]
        } else {
            &[0.0]
        };
        for (binding, shift) in image.bindings.iter().zip(shifts) {
            let translation = (geometry.center - eye + DVec3::new(*shift, 0.0, 0.0)).as_vec3();
            write_overlay_uniform(
                &self.queue,
                &binding.uniform_buffer,
                translation,
                [0.0, 0.0, 1.0, 1.0],
                image.opacity,
                pull,
            );
            draws.push((
                geometry.vertex_buffer.clone(),
                geometry.index_buffer.clone(),
                geometry.index_count,
                binding.bind_group.clone(),
            ));
        }
    }

    /// Makes sure `id` has a mesh for terrain `stamp`. A mesh built for older
    /// elevation is rebuilt while `rebuilds_left` lasts; its skirt hides the
    /// seam against already rebuilt neighbours until it is its turn.
    fn ensure_mesh(
        &mut self,
        id: TileId,
        stamp: u64,
        stitched: [u8; 4],
        corners: [u8; 4],
        leaves: &HashSet<TileId>,
        rebuilds_left: &mut usize,
    ) {
        if let Some(mesh) = self.meshes.get_mut(&id) {
            mesh.last_used_frame = self.frame_index;
            // New elevation only matters to the meshes it can change; the rest
            // keep their shape, so neighbours stay consistent while tiles stream in.
            if mesh.stitched == stitched
                && mesh.corners == corners
                && stamp != 0
                && mesh.terrain_stamp != 0
                && mesh.terrain_stamp != stamp
                && !self
                    .terrain
                    .cache()
                    .changed_since(mesh.terrain_stamp - 1, id)
            {
                mesh.terrain_stamp = stamp;
            }
        }
        match self.meshes.get(&id) {
            Some(mesh)
                if mesh.terrain_stamp == stamp
                    && mesh.stitched == stitched
                    && mesh.corners == corners =>
            {
                return;
            }
            // Flat meshes are cheap, and turning terrain off must be immediate.
            Some(_) if *rebuilds_left == 0 && stamp != 0 => return,
            Some(_) => *rebuilds_left = rebuilds_left.saturating_sub(1),
            None => {}
        }
        self.meshes_built += 1;
        let subdivisions = if stamp == 0 {
            crate::tiles::DEFAULT_SUBDIVISIONS
        } else {
            terrain_subdivisions(id.z)
        };
        let built = if stamp == 0 {
            build_tile_vertices_on(id, subdivisions, None, [0; 4], &|tile, _| tile.z)
        } else {
            build_tile_vertices_on(
                id,
                subdivisions,
                Some(&self.terrain),
                stitched,
                &|tile, corner| corner_level(tile, corner, &|other| leaves.contains(&other)),
            )
        };
        let vertices = interleave(&built.positions, &built.uvs);
        match self.meshes.get_mut(&id) {
            // Same grid, new heights: the buffers, the uniform and the texture
            // binding all stay, only the vertex data is replaced.
            Some(mesh) if mesh.subdivisions == subdivisions => {
                self.queue
                    .write_buffer(&mesh.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
                mesh.terrain_stamp = stamp;
                mesh.stitched = stitched;
                mesh.corners = corners;
            }
            _ => {
                let indices = self
                    .grid_indices
                    .entry(subdivisions)
                    .or_insert_with(|| {
                        let indices = tile_mesh_indices(subdivisions);
                        let buffer =
                            self.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("terra-globe tile grid indices"),
                                    contents: bytemuck::cast_slice(&indices),
                                    usage: wgpu::BufferUsages::INDEX,
                                });
                        (buffer, u32::try_from(indices.len()).unwrap_or(u32::MAX))
                    })
                    .clone();
                let mut gpu = create_mesh(
                    &self.device,
                    &self.tile_layout,
                    &self.sampler,
                    &self.fallback_view,
                    &vertices,
                    indices,
                    built.center_ecef,
                );
                gpu.terrain_stamp = stamp;
                gpu.stitched = stitched;
                gpu.corners = corners;
                gpu.subdivisions = subdivisions;
                gpu.last_used_frame = self.frame_index;
                self.meshes.insert(id, gpu);
            }
        }
    }

    fn ensure_flat_mesh(&mut self, id: TileId) {
        if self.flat_meshes.contains_key(&id) {
            return;
        }
        let mesh = build_flat_tile_mesh(id);
        let gpu = upload_mesh(
            &self.device,
            &self.tile_layout,
            &self.sampler,
            &self.fallback_view,
            &mesh,
        );
        self.flat_meshes.insert(id, gpu);
    }

    fn rebind_flat(&mut self, id: TileId, texture: Option<TileId>) {
        let Some(mesh) = self.flat_meshes.get(&id) else {
            return;
        };
        if mesh.bound_texture == texture {
            return;
        }
        let uniform_buffer = mesh.uniform_buffer.clone();
        let view = texture
            .and_then(|key| self.textures.get(&key))
            .map(|texture| &texture.view);
        let bind_group = create_bind_group(
            &self.device,
            &self.tile_layout,
            &self.sampler,
            &self.fallback_view,
            view,
            &uniform_buffer,
        );
        if let Some(mesh) = self.flat_meshes.get_mut(&id) {
            mesh.bind_group = bind_group;
            mesh.bound_texture = texture;
        }
    }

    fn rebind(&mut self, id: TileId, texture: Option<TileId>) {
        let Some(mesh) = self.meshes.get(&id) else {
            return;
        };
        if mesh.bound_texture == texture {
            return;
        }
        let uniform_buffer = mesh.uniform_buffer.clone();
        let view = texture
            .and_then(|key| self.textures.get(&key))
            .map(|texture| &texture.view);
        let bind_group = create_bind_group(
            &self.device,
            &self.tile_layout,
            &self.sampler,
            &self.fallback_view,
            view,
            &uniform_buffer,
        );
        if let Some(mesh) = self.meshes.get_mut(&id) {
            mesh.bind_group = bind_group;
            mesh.bound_texture = texture;
        }
    }

    /// Reads the offscreen texture back into RGBA bytes, for snapshot tests.
    pub fn read_pixels(&self) -> Result<(u32, u32, Vec<u8>), GlobeError> {
        let (width, height) = self.size;
        if width == 0 || height == 0 {
            return Err(GlobeError::ZeroSized);
        }
        let bytes_per_pixel = 4u32;
        let unpadded = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terra-globe readback"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("terra-globe readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.color_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| GlobeError::Readback(format!("device poll failed: {error:?}")))?;
        receiver
            .recv()
            .map_err(|error| GlobeError::Readback(format!("map channel closed: {error}")))?
            .map_err(|error| GlobeError::Readback(format!("map failed: {error}")))?;

        let mapped = slice
            .get_mapped_range()
            .map_err(|error| GlobeError::Readback(format!("mapped range failed: {error:?}")))?;
        let mut pixels = Vec::with_capacity((unpadded * height) as usize);
        for row in 0..height {
            let start = (row * padded) as usize;
            pixels.extend_from_slice(&mapped[start..start + unpadded as usize]);
        }
        drop(mapped);
        buffer.unmap();
        Ok((width, height, pixels))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TilePass {
    Base,
    Overlay,
}

/// The base pass overwrites with opaque imagery and writes depth. The overlay
/// pass blends over it at the same depth (hence `GreaterEqual`) without
/// writing, so stacked overlays and the scene fills still see the globe's depth.
fn tile_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    pass: TilePass,
) -> wgpu::RenderPipeline {
    let overlay = pass == TilePass::Overlay;
    let vertex_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(if overlay {
            "terra-globe overlay pipeline"
        } else {
            "terra-globe pipeline"
        }),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: 5 * 4,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &vertex_attributes,
            })],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(if overlay { "fs_overlay" } else { "fs_main" }),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: SURFACE_FORMAT,
                blend: Some(if overlay {
                    wgpu::BlendState::ALPHA_BLENDING
                } else {
                    wgpu::BlendState::REPLACE
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            // Tiles are single-sided; disabling culling avoids winding bugs
            // at the antimeridian and around the poles for negligible cost.
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(!overlay),
            depth_compare: Some(if overlay {
                wgpu::CompareFunction::GreaterEqual
            } else {
                wgpu::CompareFunction::Greater
            }),
            stencil: wgpu::StencilState::default(),
            // Image overlays sit a metre above the surface; a little bias keeps
            // their chords from losing to the base tessellation at grazing
            // angles. Smaller than the scene pipeline's, so fills stay on top.
            bias: if overlay {
                wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 1.0,
                    clamp: 0.0,
                }
            } else {
                wgpu::DepthBiasState::default()
            },
        }),
        multisample: wgpu::MultisampleState {
            count: SAMPLE_COUNT,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}

/// Missing tiles to fetch for this frame.
///
/// A tile that already has a fallback texture only needs itself: the coarse
/// imagery is on screen and the sharp tile is one round-trip away. A tile with
/// no ready ancestor at all gets its whole ancestor chain, so the first load
/// fills in progressively instead of showing nothing.
fn desired_requests(
    selection: &[crate::tiles::SelectedTile],
    ready: &HashSet<TileId>,
) -> Vec<(TileId, u8)> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for tile in selection {
        if ready.contains(&tile.id) {
            continue;
        }
        if tile.texture.is_some() {
            if seen.insert(tile.id) {
                out.push((tile.id, tile.id.z));
            }
            continue;
        }
        let mut chain = Vec::new();
        let mut current = Some(tile.id);
        while let Some(id) = current {
            chain.push(id);
            current = id.parent();
        }
        for id in chain.into_iter().rev() {
            if !ready.contains(&id) && seen.insert(id) {
                out.push((id, id.z));
            }
        }
    }
    out
}

fn sanitize_opacity(opacity: f32) -> f32 {
    if opacity.is_finite() {
        opacity.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn ingest_layer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layer: &mut TileLayer,
    frame: u64,
    fetched: Vec<FetchedTile>,
) {
    for tile in fetched {
        let Ok(image) = tile.result else {
            continue;
        };
        let view = upload_srgb_texture(
            device,
            queue,
            &mip_chain(&premultiply_srgb(&image)),
            "terra-globe overlay tile texture",
        );
        // A bind group made for the replaced texture would keep showing it.
        layer
            .bindings
            .retain(|_, binding| binding.bound_texture != Some(tile.id));
        layer.textures.insert(
            tile.id,
            GpuTexture {
                view,
                last_used_frame: frame,
            },
        );
    }
}

fn write_overlay_uniform(
    queue: &wgpu::Queue,
    uniform_buffer: &wgpu::Buffer,
    translation: glam::Vec3,
    uv_rect: [f32; 4],
    opacity: f32,
    pull: f32,
) {
    queue.write_buffer(
        uniform_buffer,
        0,
        bytemuck::bytes_of(&TileUniform {
            translation: [translation.x, translation.y, translation.z, pull],
            uv_rect,
            tint: [1.0, 1.0, 1.0, opacity],
            light: [0.0; 4],
            up: [0.0; 4],
        }),
    );
}

fn create_overlay_binding(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    fallback_view: &wgpu::TextureView,
    view: Option<&wgpu::TextureView>,
) -> OverlayBinding {
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("terra-globe overlay uniform"),
        size: std::mem::size_of::<TileUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = create_bind_group(
        device,
        layout,
        sampler,
        fallback_view,
        view,
        &uniform_buffer,
    );
    OverlayBinding {
        uniform_buffer,
        bind_group,
        bound_texture: None,
    }
}

fn upload_geometry(device: &wgpu::Device, mesh: &TileMesh) -> GpuGeometry {
    let mut vertices = Vec::with_capacity(mesh.positions.len() * 5);
    for (position, uv) in mesh.positions.iter().zip(mesh.uvs.iter()) {
        vertices.extend_from_slice(&[position[0], position[1], position[2], uv[0], uv[1]]);
    }
    GpuGeometry {
        vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terra-globe overlay vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        }),
        index_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terra-globe overlay indices"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        }),
        index_count: u32::try_from(mesh.indices.len()).unwrap_or(u32::MAX),
        center: mesh.center_ecef,
    }
}

/// Uploads a mip chain as an sRGB texture, so sampling returns linear light.
fn upload_srgb_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mips: &[image::RgbaImage],
    label: &str,
) -> wgpu::TextureView {
    let base = mips.first().map_or((1, 1), |mip| mip.dimensions());
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: base.0,
            height: base.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: u32::try_from(mips.len()).unwrap_or(1).max(1),
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, mip) in mips.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: u32::try_from(level).unwrap_or(0),
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            mip,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * mip.width()),
                rows_per_image: Some(mip.height()),
            },
            wgpu::Extent3d {
                width: mip.width(),
                height: mip.height(),
                depth_or_array_layers: 1,
            },
        );
    }
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn draw_of(mesh: &GpuMesh) -> (wgpu::Buffer, wgpu::Buffer, u32, wgpu::BindGroup) {
    (
        mesh.vertex_buffer.clone(),
        mesh.index_buffer.clone(),
        mesh.index_count,
        mesh.bind_group.clone(),
    )
}

/// Hillshading strength on terrain; flat ground stays at full brightness.
const HILLSHADE_STRENGTH: f32 = 0.6;

/// Light from the north-west at 45 degrees, the cartographic convention, in
/// the local frame at `center`: `(direction towards the light, local up)`.
fn sun_at(center: DVec3) -> ([f32; 4], [f32; 4]) {
    let (east, north, up) = crate::geo::enu_axes(center);
    let light = up * std::f64::consts::FRAC_1_SQRT_2 + (north - east) * 0.5;
    let to_array = |v: DVec3, w: f32| [v.x as f32, v.y as f32, v.z as f32, w];
    (to_array(light, HILLSHADE_STRENGTH), to_array(up, 0.0))
}

fn write_tile_uniform(
    queue: &wgpu::Queue,
    mesh: &mut GpuMesh,
    translation: glam::Vec3,
    uv_rect: [f32; 4],
    shaded: bool,
) {
    let (mut light, up) = if shaded {
        sun_at(mesh.center)
    } else {
        ([0.0; 4], [0.0; 4])
    };
    if !shaded {
        light[3] = 0.0;
    }
    queue.write_buffer(
        &mesh.uniform_buffer,
        0,
        bytemuck::bytes_of(&TileUniform {
            translation: [translation.x, translation.y, translation.z, 0.0],
            uv_rect,
            tint: [1.0, 1.0, 1.0, 1.0],
            light,
            up,
        }),
    );
}

fn create_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    fallback_view: &wgpu::TextureView,
    view: Option<&wgpu::TextureView>,
    uniform_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("terra-globe tile bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(view.unwrap_or(fallback_view)),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

/// Position and texture coordinate of each vertex, as the tile pipelines
/// read them.
fn interleave(positions: &[[f32; 3]], uvs: &[[f32; 2]]) -> Vec<f32> {
    let mut vertices = Vec::with_capacity(positions.len() * 5);
    for (position, uv) in positions.iter().zip(uvs.iter()) {
        vertices.extend_from_slice(&[position[0], position[1], position[2], uv[0], uv[1]]);
    }
    vertices
}

fn upload_mesh(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    fallback_view: &wgpu::TextureView,
    mesh: &TileMesh,
) -> GpuMesh {
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terra-globe tile indices"),
        contents: bytemuck::cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    create_mesh(
        device,
        layout,
        sampler,
        fallback_view,
        &interleave(&mesh.positions, &mesh.uvs),
        (
            index_buffer,
            u32::try_from(mesh.indices.len()).unwrap_or(u32::MAX),
        ),
        mesh.center_ecef,
    )
}

fn create_mesh(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    fallback_view: &wgpu::TextureView,
    vertices: &[f32],
    (index_buffer, index_count): (wgpu::Buffer, u32),
    center: DVec3,
) -> GpuMesh {
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terra-globe tile vertices"),
        contents: bytemuck::cast_slice(vertices),
        // Rewritten in place when the terrain under the tile changes.
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("terra-globe tile uniform"),
        size: std::mem::size_of::<TileUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = create_bind_group(
        device,
        layout,
        sampler,
        fallback_view,
        None,
        &uniform_buffer,
    );
    GpuMesh {
        vertex_buffer,
        index_buffer,
        index_count,
        center,
        uniform_buffer,
        bind_group,
        bound_texture: None,
        terrain_stamp: 0,
        stitched: [0; 4],
        corners: [0; 4],
        subdivisions: 0,
        last_used_frame: 0,
    }
}

fn create_targets(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    sample_count: u32,
) -> (
    wgpu::Texture,
    wgpu::TextureView,
    Option<wgpu::TextureView>,
    wgpu::TextureView,
) {
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("terra-globe colour target"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SURFACE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());

    let msaa = (sample_count > 1).then(|| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terra-globe msaa target"),
            size: extent,
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: SURFACE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    });
    let msaa_view =
        msaa.map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));

    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("terra-globe depth target"),
        size: extent,
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
    (color, color_view, msaa_view, depth_view)
}
