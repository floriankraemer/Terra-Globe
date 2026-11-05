//! The wgpu globe renderer.
//!
//! Renders to an offscreen `Rgba8Unorm` texture so that any UI toolkit can
//! display it (egui registers the texture; the renderer has no egui dependency).
//! Depth is reverse-Z with an infinite far plane and every transform is
//! eye-relative.

use std::collections::{HashMap, HashSet};

use glam::DVec3;
use wgpu::util::DeviceExt;

use crate::camera::Camera;
use crate::flat::{FlatCamera, SceneMode, build_flat_tile_mesh, select_flat_tiles};
use crate::geo::TileId;
use crate::imagery::{FALLBACK_RGBA, FetchedTile, TileFetcher, TileProvider};
use crate::primitives::{FillMesh, LineMesh, build_meshes};
use crate::scene::{ROUTE_COLOR, Scene, TRANSIENT_COLOR, transient_style};
use crate::tiles::{SelectionOptions, TileMesh, build_polar_cap, build_tile_mesh, select_tiles};

pub const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

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
}

struct GpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    center: DVec3,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bound_texture: Option<TileId>,
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

        let vertex_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terra-globe pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 5 * 4,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex_attributes,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: SURFACE_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
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
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: SAMPLE_COUNT,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terra-globe tile sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
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
            None,
        );
        let south_cap = upload_mesh(
            &device,
            &tile_layout,
            &sampler,
            &fallback_view,
            &build_polar_cap(false, 96, 6),
            None,
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
            let (fill, line) = build_meshes(&geometry, &style, selected, self.mode);
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

    fn prepare_scene_draws(&mut self, eye: DVec3) -> Vec<SceneDraw> {
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
                        translation: [translation.x, translation.y, translation.z, 0.0],
                        uv_rect: [0.0, 0.0, 1.0, 1.0],
                        tint: [1.0, 1.0, 1.0, 1.0],
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
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("terra-globe tile texture"),
                size: wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                texture.as_image_copy(),
                &image,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * image.width()),
                    rows_per_image: Some(image.height()),
                },
                wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.textures.insert(
                tile.id,
                GpuTexture {
                    view,
                    last_used_frame: 0,
                },
            );
        }
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
        self.frame_index += 1;
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
            select_tiles(
                camera,
                self.size,
                provider.max_level(),
                self.options,
                &|id| ready.contains(&id),
            )
        };

        let mut selected_ids = HashSet::with_capacity(selection.len());
        for tile in &selection {
            selected_ids.insert(tile.id);
            let texture_key = tile.texture.map(|(id, _)| id);
            if mode.is_flat() {
                self.ensure_flat_mesh(tile.id);
                self.rebind_flat(tile.id, texture_key);
            } else {
                self.ensure_mesh(tile.id);
                self.rebind(tile.id, texture_key);
            }
        }

        // Request every missing tile in the selection, plus its ancestors so the
        // fallback chain fills in progressively. Gating this on "no ancestor is
        // ready" was the bug that stopped deeper imagery from ever loading.
        fetcher.set_desired(provider, desired_requests(&selection, &ready));

        // Keep a little history for parent fallback when the camera jitters,
        // but never let the caches grow without bound.
        if self.meshes.len() > self.options.max_tiles.saturating_mul(2) {
            self.meshes.retain(|id, _| selected_ids.contains(id));
        }
        if self.flat_meshes.len() > self.options.max_tiles.saturating_mul(4) {
            self.flat_meshes.retain(|id, _| selected_ids.contains(id));
        }
        if self.textures.len() > self.options.max_tiles.saturating_mul(2) {
            let mut evicted = Vec::new();
            self.textures.retain(|id, texture| {
                let keep = self.frame_index.saturating_sub(texture.last_used_frame) < 240;
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
        let mut draws = Vec::with_capacity(selection.len());
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
            let (uv_rect, texture_key) = match tile.texture {
                Some((ancestor, uv)) => (uv, Some(ancestor)),
                None => ([0.0, 0.0, 1.0, 1.0], None),
            };
            if let Some(key) = texture_key {
                if let Some(texture) = self.textures.get_mut(&key) {
                    texture.last_used_frame = self.frame_index;
                }
            }
            write_tile_uniform(&self.queue, mesh, translation, uv_rect);
            draws.push(draw_of(mesh));
        }

        if !mode.is_flat() {
            for cap in [&mut self.north_cap, &mut self.south_cap] {
                let translation = (cap.center - eye).as_vec3();
                write_tile_uniform(&self.queue, cap, translation, [0.0, 0.0, 1.0, 1.0]);
                draws.push(draw_of(cap));
            }
        }

        if self.scene_dirty {
            self.rebuild_scene();
        }
        let scene_draws = self.prepare_scene_draws(eye);

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
            // The two polar caps are always drawn and are not tiles; keep the
            // number meaningful for the debug overlay and the budget tests.
            tiles_drawn: self.selection.len(),
            textures_ready: self.textures.len(),
            requests_in_flight: fetcher.in_flight_count(),
            requests_queued: fetcher.queued_count(),
            deepest_level: self
                .selection
                .iter()
                .map(|tile| tile.id.z)
                .max()
                .unwrap_or(0),
            meshes_resident: self.meshes.len(),
            scene_items: self.scene_items.len(),
        };
        self.stats
    }

    fn ensure_mesh(&mut self, id: TileId) {
        if self.meshes.contains_key(&id) {
            return;
        }
        let mesh = build_tile_mesh(id, crate::tiles::DEFAULT_SUBDIVISIONS);
        let gpu = upload_mesh(
            &self.device,
            &self.tile_layout,
            &self.sampler,
            &self.fallback_view,
            &mesh,
            None,
        );
        self.meshes.insert(id, gpu);
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
            None,
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

fn draw_of(mesh: &GpuMesh) -> (wgpu::Buffer, wgpu::Buffer, u32, wgpu::BindGroup) {
    (
        mesh.vertex_buffer.clone(),
        mesh.index_buffer.clone(),
        mesh.index_count,
        mesh.bind_group.clone(),
    )
}

fn write_tile_uniform(
    queue: &wgpu::Queue,
    mesh: &mut GpuMesh,
    translation: glam::Vec3,
    uv_rect: [f32; 4],
) {
    queue.write_buffer(
        &mesh.uniform_buffer,
        0,
        bytemuck::bytes_of(&TileUniform {
            translation: [translation.x, translation.y, translation.z, 0.0],
            uv_rect,
            tint: [1.0, 1.0, 1.0, 1.0],
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

fn upload_mesh(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    fallback_view: &wgpu::TextureView,
    mesh: &TileMesh,
    texture: Option<&wgpu::TextureView>,
) -> GpuMesh {
    let mut vertices = Vec::with_capacity(mesh.positions.len() * 5);
    for (position, uv) in mesh.positions.iter().zip(mesh.uvs.iter()) {
        vertices.extend_from_slice(&[position[0], position[1], position[2], uv[0], uv[1]]);
    }
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terra-globe tile vertices"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terra-globe tile indices"),
        contents: bytemuck::cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
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
        texture,
        &uniform_buffer,
    );
    GpuMesh {
        vertex_buffer,
        index_buffer,
        index_count: u32::try_from(mesh.indices.len()).unwrap_or(u32::MAX),
        center: mesh.center_ecef,
        uniform_buffer,
        bind_group,
        bound_texture: None,
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
