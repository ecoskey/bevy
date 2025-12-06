use crate::{
    render_resource::{encase::internal::WriteInto, DynamicUniformBuffer, ShaderType},
    renderer::{RenderDevice, RenderQueue},
    sync_component::SyncComponentPlugin,
    sync_world::RenderEntity,
    Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
};
use bevy_app::{App, Plugin};
use bevy_camera::visibility::ViewVisibility;
use bevy_ecs::{
    bundle::NoBundleEffect,
    component::Component,
    prelude::*,
    query::{QueryData, QueryFilter, QueryItem, QueryIter, ReadOnlyQueryData},
    system::{
        compose, ParamBuilder, RunSystemError, ScopeSystem, SystemRunner, SystemRunnerBuilder,
        SystemScope,
    },
};
use bevy_platform::sync::{Arc, Mutex};
use core::{marker::PhantomData, ops::Deref};
use std::ops::DerefMut;
use tracing::debug;

pub use bevy_render_macros::ExtractComponent;

/// Stores the index of a uniform inside of [`ComponentUniforms`].
#[derive(Component)]
pub struct DynamicUniformIndex<C: Component> {
    index: u32,
    marker: PhantomData<C>,
}

impl<C: Component> DynamicUniformIndex<C> {
    #[inline]
    pub fn index(&self) -> u32 {
        self.index
    }
}

/// Describes how a component gets extracted for rendering.
///
/// Therefore the component is transferred from the "app world" into the "render world"
/// in the [`ExtractSchedule`] step.
pub trait ExtractComponent: Component {
    /// ECS [`ReadOnlyQueryData`] to fetch the components to extract.
    type QueryData: ReadOnlyQueryData;
    /// Filters the entities with additional constraints.
    type QueryFilter: QueryFilter;

    /// The output from extraction.
    ///
    /// Returning `None` based on the queried item will remove the component from the entity in
    /// the render world. This can be used, for example, to conditionally extract camera settings
    /// in order to disable a rendering feature on the basis of those settings, without removing
    /// the component from the entity in the main world.
    ///
    /// The output may be different from the queried component.
    /// This can be useful for example if only a subset of the fields are useful
    /// in the render world.
    ///
    /// `Out` has a [`Bundle`] trait bound instead of a [`Component`] trait bound in order to allow use cases
    /// such as tuples of components as output.
    type Out: Bundle<Effect: NoBundleEffect>;

    // TODO: https://github.com/rust-lang/rust/issues/29661
    // type Out: Component = Self;

    /// Defines how the component is transferred into the "render world".
    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out>;
}

/// This plugin prepares the components of the corresponding type for the GPU
/// by transforming them into uniforms.
///
/// They can then be accessed from the [`ComponentUniforms`] resource.
/// For referencing the newly created uniforms a [`DynamicUniformIndex`] is inserted
/// for every processed entity.
///
/// Therefore it sets up the [`RenderSystems::Prepare`] step
/// for the specified [`ExtractComponent`].
pub struct UniformComponentPlugin<C>(PhantomData<fn() -> C>);

impl<C> Default for UniformComponentPlugin<C> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<C: Component + ShaderType + WriteInto + Clone> Plugin for UniformComponentPlugin<C> {
    fn build(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .insert_resource(ComponentUniforms::<C>::default())
                .add_systems(
                    Render,
                    prepare_uniform_components::<C>.in_set(RenderSystems::PrepareResources),
                );
        }
    }
}

/// Stores all uniforms of the component type.
#[derive(Resource)]
pub struct ComponentUniforms<C: Component + ShaderType> {
    uniforms: DynamicUniformBuffer<C>,
}

impl<C: Component + ShaderType> Deref for ComponentUniforms<C> {
    type Target = DynamicUniformBuffer<C>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.uniforms
    }
}

impl<C: Component + ShaderType> ComponentUniforms<C> {
    #[inline]
    pub fn uniforms(&self) -> &DynamicUniformBuffer<C> {
        &self.uniforms
    }
}

impl<C: Component + ShaderType> Default for ComponentUniforms<C> {
    fn default() -> Self {
        Self {
            uniforms: Default::default(),
        }
    }
}

/// This system prepares all components of the corresponding component type.
/// They are transformed into uniforms and stored in the [`ComponentUniforms`] resource.
fn prepare_uniform_components<C>(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut component_uniforms: ResMut<ComponentUniforms<C>>,
    components: Query<(Entity, &C)>,
) where
    C: Component + ShaderType + WriteInto + Clone,
{
    let components_iter = components.iter();
    let count = components_iter.len();
    let Some(mut writer) =
        component_uniforms
            .uniforms
            .get_writer(count, &render_device, &render_queue)
    else {
        return;
    };
    let entities = components_iter
        .map(|(entity, component)| {
            (
                entity,
                DynamicUniformIndex::<C> {
                    index: writer.write(component),
                    marker: PhantomData,
                },
            )
        })
        .collect::<Vec<_>>();
    commands.try_insert_batch(entities);
}

pub struct QueryIn<'i, D: QueryData, F: QueryFilter> {
    data: QueryItem<'i, 'i, D>,
    _filter: PhantomData<fn(F)>,
}

impl<D: QueryData, F: QueryFilter> SystemInput for QueryIn<'_, D, F> {
    type Param<'i> = QueryIn<'i, D, F>;

    type Inner<'i> = QueryItem<'i, 'i, D>;

    fn wrap(this: Self::Inner<'_>) -> Self::Param<'_> {
        QueryIn {
            data: this,
            _filter: PhantomData,
        }
    }
}

impl<'i, D: QueryData, F: QueryFilter> Deref for QueryIn<'i, D, F> {
    type Target = QueryItem<'i, 'i, D>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'i, D: QueryData, F: QueryFilter> DerefMut for QueryIn<'i, D, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

pub struct ExtractPlugin<D, F, B, Sys>
where
    Sys: ScopeSystem,
{
    system: Arc<Mutex<Option<Sys>>>,
    _data: PhantomData<fn(D, F) -> B>,
}

impl<D, F, B, Sys> ExtractPlugin<D, F, B, Sys>
where
    D: ReadOnlyQueryData,
    F: QueryFilter,
    B: Bundle<Effect: NoBundleEffect>,
    Sys: ScopeSystem<In = QueryIn<'static, D, F>, Out = Option<B>>,
{
    pub fn new<Marker>(
        system: impl IntoSystem<QueryIn<'static, D, F>, Option<B>, Marker, System = Sys>,
    ) -> Self {
        Self {
            system: Arc::new(Mutex::new(Some(IntoSystem::into_system(system)))),
            _data: PhantomData,
        }
    }
}

impl<D, F, B, Sys> Plugin for ExtractPlugin<D, F, B, Sys>
where
    D: ReadOnlyQueryData + 'static,
    F: QueryFilter + 'static,
    B: Bundle<Effect: NoBundleEffect>,
    Sys: ScopeSystem<In = QueryIn<'static, D, F>, Out = Option<B>>,
{
    fn build(&self, app: &mut App) {
        let system = self.system.lock().unwrap().take().unwrap();

        app.add_systems(
            ExtractSchedule,
            (
                ParamBuilder,
                ParamBuilder,
                ParamBuilder,
                ParamBuilder::system(system),
            )
                .build_system(extract_system::<D, F, B, Sys>),
        );
    }
}

fn extract_system<D, F, B, Sys>(
    mut commands: Commands,
    mut previous_len: Local<usize>,
    query: Extract<Query<(RenderEntity, D), F>>,
    mut system: SystemRunner<QueryIn<'static, D, F>, Option<B>, Sys>,
) -> Result<(), RunSystemError>
where
    D: ReadOnlyQueryData,
    F: QueryFilter,
    B: Bundle<Effect: NoBundleEffect>,
    Sys: ScopeSystem<In = QueryIn<'static, D, F>, Out = Option<B>>,
{
    let mut values = Vec::with_capacity(*previous_len);
    let mut scope = system.scope()?;
    for (entity, data) in query.into_iter() {
        let bundle = scope.run_with(data)?;
        if let Some(bundle) = bundle {
            values.push((entity, bundle));
        } else {
            commands.entity(entity).remove::<B>();
        }
    }

    *previous_len = values.len();
    commands.try_insert_batch(values);
    Ok(())
}

pub fn extract_cloned<C: Component + Clone, F: QueryFilter>(input: QueryIn<&C, F>) -> Option<C> {
    Some((*input).clone())
}

/// This plugin extracts the components into the render world for synced entities.
///
/// To do so, it sets up the [`ExtractSchedule`] step for the specified [`ExtractComponent`].
pub struct ExtractComponentPlugin<C, F = ()> {
    only_extract_visible: bool,
    marker: PhantomData<fn() -> (C, F)>,
}

impl<C, F> Default for ExtractComponentPlugin<C, F> {
    fn default() -> Self {
        Self {
            only_extract_visible: false,
            marker: PhantomData,
        }
    }
}

impl<C, F> ExtractComponentPlugin<C, F> {
    pub fn extract_visible() -> Self {
        Self {
            only_extract_visible: true,
            marker: PhantomData,
        }
    }
}

impl<C: ExtractComponent> Plugin for ExtractComponentPlugin<C> {
    fn build(&self, app: &mut App) {
        app.add_plugins(SyncComponentPlugin::<C>::default());

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            if self.only_extract_visible {
                render_app.add_systems(ExtractSchedule, extract_visible_components::<C>);
            } else {
                render_app.add_systems(ExtractSchedule, extract_components::<C>);
            }
        }
    }
}

/// This system extracts all components of the corresponding [`ExtractComponent`], for entities that are synced via [`crate::sync_world::SyncToRenderWorld`].
fn extract_components<C: ExtractComponent>(
    mut commands: Commands,
    mut previous_len: Local<usize>,
    query: Extract<Query<(RenderEntity, C::QueryData), C::QueryFilter>>,
) {
    let mut values = Vec::with_capacity(*previous_len);
    for (entity, query_item) in &query {
        if let Some(component) = C::extract_component(query_item) {
            values.push((entity, component));
        } else {
            commands.entity(entity).remove::<C::Out>();
        }
    }
    *previous_len = values.len();
    commands.try_insert_batch(values);
}

/// This system extracts all components of the corresponding [`ExtractComponent`], for entities that are visible and synced via [`crate::sync_world::SyncToRenderWorld`].
fn extract_visible_components<C: ExtractComponent>(
    mut commands: Commands,
    mut previous_len: Local<usize>,
    query: Extract<Query<(RenderEntity, &ViewVisibility, C::QueryData), C::QueryFilter>>,
) {
    let mut values = Vec::with_capacity(*previous_len);
    for (entity, view_visibility, query_item) in &query {
        if view_visibility.get() {
            if let Some(component) = C::extract_component(query_item) {
                values.push((entity, component));
            } else {
                commands.entity(entity).remove::<C::Out>();
            }
        }
    }
    *previous_len = values.len();
    commands.try_insert_batch(values);
}
