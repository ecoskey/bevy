use crate::FullscreenShader;

use super::{settings::BloomUniforms, Bloom, BloomCompositeMode, BLOOM_TEXTURE_FORMAT};
use bevy_asset::load_embedded_asset;
use bevy_ecs::{
    error::BevyError,
    prelude::{Component, Entity},
    resource::Resource,
    system::{Commands, Query, Res, ResMut},
    world::{FromWorld, World},
};
use bevy_math::Vec2;
use bevy_render::{
    render_resource::{
        binding_types::{sampler, texture_2d, uniform_buffer},
        *,
    },
    renderer::RenderDevice,
    view::ViewTarget,
};
use bevy_utils::default;

#[derive(Component)]
pub struct BloomDownsamplingPipelineIds {
    pub main: CachedRenderPipelineId,
    pub first: CachedRenderPipelineId,
}

#[derive(Resource)]
pub struct BloomDownsamplingPipeline {
    /// Layout with a texture, a sampler, and uniforms
    pub bind_group_layout: BindGroupLayout,
    pub sampler: Sampler,
    pub specialized_cache: SpecializedCache<RenderPipeline, BloomDownsamplingSpecializer>,
}

pub struct BloomDownsamplingSpecializer;

#[derive(PartialEq, Eq, Hash, Clone, SpecializerKey)]
pub struct BloomDownsamplingKey {
    prefilter: bool,
    first_downsample: bool,
    uniform_scale: bool,
}

impl FromWorld for BloomDownsamplingPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        // Bind group layout
        let bind_group_layout = render_device.create_bind_group_layout(
            "bloom_downsampling_bind_group_layout_with_settings",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    // Input texture binding
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // Sampler binding
                    sampler(SamplerBindingType::Filtering),
                    // Downsampling settings binding
                    uniform_buffer::<BloomUniforms>(true),
                ),
            ),
        );

        // Sampler
        let sampler = render_device.create_sampler(&SamplerDescriptor {
            min_filter: FilterMode::Linear,
            mag_filter: FilterMode::Linear,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            ..Default::default()
        });

        let fullscreen_shader = world.resource::<FullscreenShader>().clone();
        let fragment_shader = load_embedded_asset!(world, "bloom.wgsl");
        let base_descriptor = RenderPipelineDescriptor {
            layout: vec![bind_group_layout.clone()],
            vertex: fullscreen_shader.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: fragment_shader.clone(),
                targets: vec![Some(ColorTargetState {
                    format: BLOOM_TEXTURE_FORMAT,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        };

        let specialized_cache =
            SpecializedCache::new(BloomDownsamplingSpecializer, None, base_descriptor);

        BloomDownsamplingPipeline {
            bind_group_layout,
            sampler,
            specialized_cache,
        }
    }
}

impl Specializer<RenderPipeline> for BloomDownsamplingSpecializer {
    type Key = BloomDownsamplingKey;

    fn specialize(
        &self,
        key: Self::Key,
        descriptor: &mut RenderPipelineDescriptor,
    ) -> Result<Canonical<Self::Key>, BevyError> {
        descriptor.label = Some(if key.first_downsample {
            "bloom_downsampling_pipeline_first".into()
        } else {
            "bloom_downsampling_pipeline".into()
        });

        let fragment = descriptor.fragment_mut()?;

        fragment.entry_point = Some(if key.first_downsample {
            "downsample_first".into()
        } else {
            "downsample".into()
        });

        let shader_defs = &mut fragment.shader_defs;

        if key.first_downsample {
            shader_defs.push("FIRST_DOWNSAMPLE".into());
        }

        if key.prefilter {
            shader_defs.push("USE_THRESHOLD".into());
        }

        if key.uniform_scale {
            shader_defs.push("UNIFORM_SCALE".into());
        }

        Ok(key)
    }
}

pub fn prepare_downsampling_pipeline(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipeline: ResMut<BloomDownsamplingPipeline>,
    views: Query<(Entity, &Bloom)>,
) -> Result<(), BevyError> {
    for (entity, bloom) in &views {
        let prefilter = bloom.prefilter.threshold > 0.0;

        let pipeline_id = pipeline.specialized_cache.specialize(
            &pipeline_cache,
            BloomDownsamplingKey {
                prefilter,
                first_downsample: false,
                uniform_scale: bloom.scale == Vec2::ONE,
            },
        )?;

        let pipeline_first_id = pipeline.specialized_cache.specialize(
            &pipeline_cache,
            BloomDownsamplingKey {
                prefilter,
                first_downsample: true,
                uniform_scale: bloom.scale == Vec2::ONE,
            },
        )?;

        commands
            .entity(entity)
            .insert(BloomDownsamplingPipelineIds {
                first: pipeline_first_id,
                main: pipeline_id,
            });
    }
    Ok(())
}

#[derive(Component)]
pub struct UpsamplingPipelineIds {
    pub id_main: CachedRenderPipelineId,
    pub id_final: CachedRenderPipelineId,
}

#[derive(Resource)]
pub struct BloomUpsamplingPipeline {
    pub bind_group_layout: BindGroupLayout,
    pub specialized_cache: SpecializedCache<RenderPipeline, BloomUpsamplingSpecializer>,
}

impl FromWorld for BloomUpsamplingPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let bind_group_layout = render_device.create_bind_group_layout(
            "bloom_upsampling_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    // Input texture
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // Sampler
                    sampler(SamplerBindingType::Filtering),
                    // BloomUniforms
                    uniform_buffer::<BloomUniforms>(true),
                ),
            ),
        );

        let fullscreen_shader = world.resource::<FullscreenShader>().clone();
        let fragment_shader = load_embedded_asset!(world, "bloom.wgsl");
        let base_descriptor = RenderPipelineDescriptor {
            label: Some("bloom_upsampling_pipeline".into()),
            layout: vec![bind_group_layout.clone()],
            vertex: fullscreen_shader.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: fragment_shader.clone(),
                entry_point: Some("upsample".into()),
                ..default()
            }),
            ..default()
        };

        let specialized_cache =
            SpecializedCache::new(BloomUpsamplingSpecializer, None, base_descriptor);

        BloomUpsamplingPipeline {
            bind_group_layout,
            specialized_cache,
        }
    }
}

pub struct BloomUpsamplingSpecializer;

#[derive(PartialEq, Eq, Hash, Clone, SpecializerKey)]
pub struct BloomUpsamplingKey {
    composite_mode: BloomCompositeMode,
    final_pipeline: bool,
}

impl Specializer<RenderPipeline> for BloomUpsamplingSpecializer {
    type Key = BloomUpsamplingKey;

    fn specialize(
        &self,
        key: Self::Key,
        descriptor: &mut RenderPipelineDescriptor,
    ) -> Result<Canonical<Self::Key>, BevyError> {
        let texture_format = if key.final_pipeline {
            ViewTarget::TEXTURE_FORMAT_HDR
        } else {
            BLOOM_TEXTURE_FORMAT
        };

        let color_blend = match key.composite_mode {
            BloomCompositeMode::EnergyConserving => {
                // At the time of developing this we decided to blend our
                // blur pyramid levels using native WGPU render pass blend
                // constants. They are set in the bloom node's run function.
                // This seemed like a good approach at the time which allowed
                // us to perform complex calculations for blend levels on the CPU,
                // however, we missed the fact that this prevented us from using
                // textures to customize bloom appearance on individual parts
                // of the screen and create effects such as lens dirt or
                // screen blur behind certain UI elements.
                //
                // TODO: Use alpha instead of blend constants and move
                // compute_blend_factor to the shader. The shader
                // will likely need to know current mip number or
                // mip "angle" (original texture is 0deg, max mip is 90deg)
                // so make sure you give it that as a uniform.
                // That does have to be provided per each pass unlike other
                // uniforms that are set once.
                BlendComponent {
                    src_factor: BlendFactor::Constant,
                    dst_factor: BlendFactor::OneMinusConstant,
                    operation: BlendOperation::Add,
                }
            }
            BloomCompositeMode::Additive => BlendComponent {
                src_factor: BlendFactor::Constant,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
        };

        let target = ColorTargetState {
            format: texture_format,
            blend: Some(BlendState {
                color: color_blend,
                alpha: BlendComponent {
                    src_factor: BlendFactor::Zero,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            }),
            write_mask: ColorWrites::ALL,
        };

        descriptor.fragment_mut()?.set_target(0, target);

        Ok(key)
    }
}

pub fn prepare_upsampling_pipeline(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipeline: ResMut<BloomUpsamplingPipeline>,
    views: Query<(Entity, &Bloom)>,
) -> Result<(), BevyError> {
    for (entity, bloom) in &views {
        let pipeline_id = pipeline.specialized_cache.specialize(
            &pipeline_cache,
            BloomUpsamplingKey {
                composite_mode: bloom.composite_mode,
                final_pipeline: false,
            },
        )?;

        let pipeline_final_id = pipeline.specialized_cache.specialize(
            &pipeline_cache,
            BloomUpsamplingKey {
                composite_mode: bloom.composite_mode,
                final_pipeline: true,
            },
        )?;

        commands.entity(entity).insert(UpsamplingPipelineIds {
            id_main: pipeline_id,
            id_final: pipeline_final_id,
        });
    }
    Ok(())
}
