//! An ambient background computed on the GPU — the prototype's reason to be on
//! WebGPU rather than WebGL2.
//!
//! A compute shader (`assets/shaders/vellum_field.wgsl`) runs Conway's Game of
//! Life into a storage texture every frame; a single full-bleed `Sprite`
//! displays it behind the whole UI. WebGL2 has no compute stage at all, so this
//! module simply cannot run on the old backend — it is the concrete payoff of
//! the `webgpu` feature flip in `Cargo.toml`.
//!
//! The structure mirrors Bevy's canonical compute example: the simulation lives
//! in the render world (a render-graph node dispatches the passes), while the
//! main world only owns the two texture handles and the colour uniform and
//! extracts them across each frame. We keep two textures and ping-pong between
//! them because each cell's next state depends on its neighbours' current state.

use std::borrow::Cow;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_graph::{self, RenderGraph, RenderLabel},
        render_resource::{
            binding_types::{texture_storage_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::GpuImage,
        Render, RenderApp, RenderStartup, RenderSystems,
    },
    shader::PipelineCacheError,
};

use crate::theme;

const SHADER_ASSET_PATH: &str = "shaders/vellum_field.wgsl";

/// Simulation grid. A multiple of `WORKGROUP_SIZE` so the dispatch covers every
/// cell exactly. The sprite is scaled far larger than this, and the texture is
/// sampled linearly, so the coarse grid reads as a soft drifting glow rather
/// than sharp pixels behind the text.
const SIZE: UVec2 = UVec2::new(192, 192);
const WORKGROUP_SIZE: u32 = 8;

/// World-space size of the background sprite. Large enough to cover any sensible
/// window; dead cells are transparent, so overscan past the viewport is free.
const COVER: f32 = 4000.0;

pub struct FieldComputePlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct FieldLabel;

/// Marks the single background sprite so `switch_field_textures` can find it
/// with a `Single` query without tripping over any other sprite.
#[derive(Component)]
struct BackgroundField;

impl Plugin for FieldComputePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_field)
            .add_systems(Update, switch_field_textures)
            .add_plugins((
                ExtractResourcePlugin::<FieldImages>::default(),
                ExtractResourcePlugin::<FieldUniforms>::default(),
            ));

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .add_systems(RenderStartup, init_field_pipeline)
            .add_systems(
                Render,
                prepare_bind_group.in_set(RenderSystems::PrepareBindGroups),
            );

        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();
        render_graph.add_node(FieldLabel, FieldNode::default());
        // Run the compute pass before the camera draws, so the sprite shows the
        // texture this frame's pass just wrote.
        render_graph.add_node_edge(FieldLabel, bevy::render::graph::CameraDriverLabel);
    }
}

/// Build the two storage textures, spawn the background sprite, and seed the
/// resources the render world will extract. Reuses the camera spawned in
/// `ui::setup` — we deliberately do not add a second one.
fn setup_field(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_target_texture(SIZE.x, SIZE.y, TextureFormat::Rgba32Float, None);
    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;
    let texture_a = images.add(image.clone());
    let texture_b = images.add(image);

    // z = -1 keeps the sprite behind 2D content; bevy_ui composites on top of
    // the 2D pass regardless, so the whole interface still draws over it.
    commands.spawn((
        BackgroundField,
        Sprite {
            image: texture_a.clone(),
            custom_size: Some(Vec2::splat(COVER)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));

    commands.insert_resource(FieldImages {
        texture_a,
        texture_b,
    });
    commands.insert_resource(FieldUniforms {
        // Living cells glow in the docs accent colour. The shader expects linear
        // RGBA, so convert from the sRGB palette constant.
        alive_color: theme::ACCENT.to_linear(),
    });
}

/// Flip the displayed texture each frame so the sprite always shows the one the
/// compute pass just wrote, rather than every other generation.
fn switch_field_textures(
    images: Res<FieldImages>,
    sprite: Single<&mut Sprite, With<BackgroundField>>,
) {
    let mut sprite = sprite.into_inner();
    sprite.image = if sprite.image == images.texture_a {
        images.texture_b.clone()
    } else {
        images.texture_a.clone()
    };
}

/// The two ping-pong textures the compute pass alternates between; each cell's
/// next state depends on its neighbours' current state, so we cannot write in
/// place.
#[derive(Resource, Clone, ExtractResource)]
struct FieldImages {
    texture_a: Handle<Image>,
    texture_b: Handle<Image>,
}

#[derive(Resource, Clone, ExtractResource, ShaderType)]
struct FieldUniforms {
    alive_color: LinearRgba,
}

#[derive(Resource)]
struct FieldBindGroups([BindGroup; 2]);

fn prepare_bind_group(
    mut commands: Commands,
    pipeline: Res<FieldPipeline>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    field_images: Res<FieldImages>,
    field_uniforms: Res<FieldUniforms>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    queue: Res<RenderQueue>,
) {
    let view_a = gpu_images.get(&field_images.texture_a).unwrap();
    let view_b = gpu_images.get(&field_images.texture_b).unwrap();

    let mut uniform_buffer = UniformBuffer::from(field_uniforms.into_inner());
    uniform_buffer.write_buffer(&render_device, &queue);

    let layout = pipeline_cache.get_bind_group_layout(&pipeline.texture_bind_group_layout);
    // Two bind groups with the textures swapped: one reads A→writes B, the other
    // reads B→writes A. The node alternates between them to advance the sim.
    let bind_group_0 = render_device.create_bind_group(
        None,
        &layout,
        &BindGroupEntries::sequential((&view_a.texture_view, &view_b.texture_view, &uniform_buffer)),
    );
    let bind_group_1 = render_device.create_bind_group(
        None,
        &layout,
        &BindGroupEntries::sequential((&view_b.texture_view, &view_a.texture_view, &uniform_buffer)),
    );
    commands.insert_resource(FieldBindGroups([bind_group_0, bind_group_1]));
}

#[derive(Resource)]
struct FieldPipeline {
    texture_bind_group_layout: BindGroupLayoutDescriptor,
    init_pipeline: CachedComputePipelineId,
    update_pipeline: CachedComputePipelineId,
}

fn init_field_pipeline(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let texture_bind_group_layout = BindGroupLayoutDescriptor::new(
        "FieldImages",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_storage_2d(TextureFormat::Rgba32Float, StorageTextureAccess::ReadOnly),
                texture_storage_2d(TextureFormat::Rgba32Float, StorageTextureAccess::WriteOnly),
                uniform_buffer::<FieldUniforms>(false),
            ),
        ),
    );
    let shader = asset_server.load(SHADER_ASSET_PATH);
    let init_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        layout: vec![texture_bind_group_layout.clone()],
        shader: shader.clone(),
        entry_point: Some(Cow::from("init")),
        ..default()
    });
    let update_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        layout: vec![texture_bind_group_layout.clone()],
        shader,
        entry_point: Some(Cow::from("update")),
        ..default()
    });

    commands.insert_resource(FieldPipeline {
        texture_bind_group_layout,
        init_pipeline,
        update_pipeline,
    });
}

/// Drives the simulation forward: wait for the shader to compile, run `init`
/// once, then alternate `update` passes, flipping which bind group is active.
enum FieldState {
    Loading,
    Init,
    Update(usize),
}

struct FieldNode {
    state: FieldState,
}

impl Default for FieldNode {
    fn default() -> Self {
        Self {
            state: FieldState::Loading,
        }
    }
}

impl render_graph::Node for FieldNode {
    fn update(&mut self, world: &mut World) {
        let pipeline = world.resource::<FieldPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        match self.state {
            FieldState::Loading => {
                match pipeline_cache.get_compute_pipeline_state(pipeline.init_pipeline) {
                    CachedPipelineState::Ok(_) => self.state = FieldState::Init,
                    // Shader still loading from disk/network — just wait.
                    CachedPipelineState::Err(PipelineCacheError::ShaderNotLoaded(_)) => {}
                    CachedPipelineState::Err(err) => {
                        panic!("Initializing assets/{SHADER_ASSET_PATH}:\n{err}")
                    }
                    _ => {}
                }
            }
            FieldState::Init => {
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.update_pipeline)
                {
                    self.state = FieldState::Update(1);
                }
            }
            FieldState::Update(0) => self.state = FieldState::Update(1),
            FieldState::Update(1) => self.state = FieldState::Update(0),
            FieldState::Update(_) => unreachable!(),
        }
    }

    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let bind_groups = &world.resource::<FieldBindGroups>().0;
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<FieldPipeline>();

        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor::default());

        match self.state {
            FieldState::Loading => {}
            FieldState::Init => {
                let init = pipeline_cache
                    .get_compute_pipeline(pipeline.init_pipeline)
                    .unwrap();
                pass.set_bind_group(0, &bind_groups[0], &[]);
                pass.set_pipeline(init);
                pass.dispatch_workgroups(SIZE.x / WORKGROUP_SIZE, SIZE.y / WORKGROUP_SIZE, 1);
            }
            FieldState::Update(index) => {
                let update = pipeline_cache
                    .get_compute_pipeline(pipeline.update_pipeline)
                    .unwrap();
                pass.set_bind_group(0, &bind_groups[index], &[]);
                pass.set_pipeline(update);
                pass.dispatch_workgroups(SIZE.x / WORKGROUP_SIZE, SIZE.y / WORKGROUP_SIZE, 1);
            }
        }

        Ok(())
    }
}
