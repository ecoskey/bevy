use std::marker::PhantomData;

use bevy_ecs::world::World;
use bevy_utils::{all_tuples, HashMap, HashSet};

use crate::{render_graph::InternedRenderLabel, renderer::RenderDevice};

use bind_group::RenderBindGroup;

use super::RenderGraph;

pub mod bind_group;
pub mod buffer;
mod encoder;
pub mod pipeline;
pub mod texture;

pub trait RenderResource: Sized + Send + Sync + 'static {
    type Descriptor: Send + Sync + 'static;
    type Data: Send + Sync + 'static;
    type Store: RenderStore<Self>;

    fn get_store(graph: &RenderGraph) -> &Self::Store; //todo: proper generic resource table
    fn get_store_mut(graph: &mut RenderGraph) -> &mut Self::Store;

    fn from_data<'a>(data: &'a Self::Data, world: &'a World) -> Option<&'a Self>;
}

pub trait RenderStore<R: RenderResource>: Send + Sync + 'static {
    fn insert(&mut self, key: u16, data: RenderResourceInit<R>);

    fn get<'a>(&'a self, world: &'a World, key: u16) -> Option<&'a RenderResourceMeta<R>>;

    fn init_queued_resources(&mut self, world: &mut World, device: &RenderDevice);
}

pub trait RetainedRenderStore<R: RenderResource>: RenderStore<R> {
    fn mark_retain(&mut self, key: u16, label: InternedRenderLabel);

    fn get_retained(&mut self, label: InternedRenderLabel) -> Option<&RenderResourceMeta<R>>;
}

pub trait WriteRenderResource: RenderResource {}

pub trait RetainedRenderResource: WriteRenderResource
where
    <Self as RenderResource>::Store: RetainedRenderStore<Self>,
{
}

pub struct RenderResourceMeta<R: RenderResource> {
    pub(super) descriptor: Option<R::Descriptor>,
    pub(super) resource: R::Data,
}

type DeferredResourceInit<R> =
    Box<dyn FnOnce(&mut World, &RenderDevice) -> RenderResourceMeta<R> + Send + Sync + 'static>;

pub enum RenderResourceInit<R: RenderResource> {
    Eager(RenderResourceMeta<R>),
    Deferred(DeferredResourceInit<R>),
}

pub struct SimpleResourceStore<R: RenderResource> {
    last_frame_resources: HashMap<InternedRenderLabel, RenderResourceMeta<R>>,
    current_resources: HashMap<u16, RenderResourceMeta<R>>,
    queued_resources: HashMap<u16, DeferredResourceInit<R>>,
    resources_to_save: HashMap<u16, InternedRenderLabel>,
}

impl<R: RenderResource> RenderStore<R> for SimpleResourceStore<R> {
    fn insert(&mut self, key: u16, data: RenderResourceInit<R>) {
        todo!()
    }

    fn get<'a>(&'a self, world: &'a World, key: u16) -> Option<&'a RenderResourceMeta<R>> {
        todo!()
    }

    fn init_queued_resources(&mut self, world: &mut World, device: &RenderDevice) {
        todo!()
    }
}

impl<R: RenderResource> RetainedRenderStore<R> for SimpleResourceStore<R> {
    fn mark_retain(&mut self, key: u16, label: InternedRenderLabel) {
        todo!()
    }

    fn get_retained(&mut self, label: InternedRenderLabel) -> Option<&RenderResourceMeta<R>> {
        todo!()
    }
}

impl<R: RenderResource> Default for SimpleResourceStore<R> {
    fn default() -> Self {
        Self {
            last_frame_resources: Default::default(),
            current_resources: Default::default(),
            queued_resources: Default::default(),
            resources_to_save: Default::default(),
        }
    }
}

impl<R: RenderResource> SimpleResourceStore<R> {
    pub fn insert(&mut self, key: RenderResourceId, resource: RenderResourceInit<R>) {
        match resource {
            RenderResourceInit::Eager(meta) => {
                self.current_resources.insert(key.index, meta);
            }
            RenderResourceInit::Deferred(init) => {
                self.queued_resources.insert(key.index, init);
            }
        };
    }

    pub fn init_deferred(&mut self, world: &mut World, render_device: &RenderDevice) {
        for (id, init) in self.queued_resources.drain() {
            self.current_resources
                .insert(id, (init)(world, render_device));
        }
    }

    pub fn get_data(&self, key: RenderResourceId) -> Option<&RenderResourceMeta<R>> {
        self.current_resources.get(&key.index)
    }

    pub fn reset(&mut self) {
        self.last_frame_resources.clear();
        for (id, meta) in self.current_resources.drain() {
            if let Some(label) = self.resources_to_save.get(&id) {
                self.last_frame_resources.insert(*label, meta);
            }
        }
    }
}

pub trait IntoRenderResource {
    type Resource: RenderResource;

    fn into_render_resource(
        self,
        world: &World,
        render_device: &RenderDevice,
    ) -> RenderResourceInit<Self::Resource>;
}

impl<R: RenderResource<Data = R>, F: FnOnce(&RenderDevice) -> R> IntoRenderResource for F {
    type Resource = R;

    fn into_render_resource(
        self,
        _world: &World,
        render_device: &RenderDevice,
    ) -> RenderResourceInit<Self::Resource> {
        RenderResourceInit::Eager(RenderResourceMeta {
            descriptor: None,
            resource: (self)(render_device),
        })
    }
}

pub struct RenderHandle<R: RenderResource> {
    pub(super) id: RenderResourceId,
    pub(super) data: PhantomData<R>,
}

impl<T: RenderResource> Copy for RenderHandle<T> {}
impl<T: RenderResource> Clone for RenderHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

pub enum RenderDependency {
    Read(RenderResourceId),
    ReadWrite(RenderResourceId),
    BindGroup(RenderBindGroup),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct RenderResourceId {
    pub(super) index: u16,
    pub(super) generation: u16,
}

#[derive(Default)]
pub struct RenderDependencies {
    reads: HashSet<RenderResourceId>,
    writes: HashSet<RenderResourceId>,
    bind_groups: HashSet<RenderBindGroup>,
}

impl RenderDependencies {
    pub fn of<'a>(deps: impl IntoRenderDependencies<'a>) -> Self {
        let mut dep_set = Self::default();
        dep_set.add_many(deps);
        dep_set
    }

    pub fn add<'a>(&'a mut self, dependency: impl IntoRenderDependency<'a>) -> &'a mut Self {
        match dependency.into_render_dependency() {
            RenderDependency::Read(id) => self.reads.insert(id),
            RenderDependency::ReadWrite(id) => self.writes.insert(id),
            RenderDependency::BindGroup(bg) => self.bind_groups.insert(bg),
        };
        self
    }

    pub fn add_many<'s, 'd: 's>(
        &'s mut self,
        dependencies: impl IntoRenderDependencies<'d>,
    ) -> &'s mut Self {
        for dep in dependencies.into_render_dependencies() {
            match dep {
                RenderDependency::Read(id) => self.reads.insert(id),
                RenderDependency::ReadWrite(id) => self.writes.insert(id),
                RenderDependency::BindGroup(bg) => self.bind_groups.insert(bg),
            };
        }
        self
    }

    pub fn contains_resource<R: RenderResource>(&self, resource: RenderHandle<R>) -> bool {
        self.reads.contains(&resource.id) || self.writes.contains(&resource.id)
    }

    pub fn contains_bind_group(&self, bind_group: RenderBindGroup) -> bool {
        self.bind_groups.contains(&bind_group)
    }
}

pub fn render_deps<'a>(deps: impl IntoRenderDependencies<'a>) -> RenderDependencies {
    RenderDependencies::of(deps)
}

pub trait IntoRenderDependency<'a>: 'a {
    fn into_render_dependency(self) -> RenderDependency;
}

impl<'a, R: RenderResource> IntoRenderDependency<'a> for &'a RenderHandle<R> {
    fn into_render_dependency(self) -> RenderDependency {
        RenderDependency::Read(self.id)
    }
}

impl<'a, R: WriteRenderResource> IntoRenderDependency<'a> for &'a mut RenderHandle<R> {
    fn into_render_dependency(self) -> RenderDependency {
        let dep = RenderDependency::ReadWrite(self.id);
        self.id.generation += 1;
        dep
    }
}

impl<'a> IntoRenderDependency<'a> for &'a RenderBindGroup {
    fn into_render_dependency(self) -> RenderDependency {
        RenderDependency::BindGroup(*self)
    }
}

pub trait IntoRenderDependencies<'a>: 'a {
    fn into_render_dependencies(self) -> impl Iterator<Item = RenderDependency>;
}

impl<'a, T: IntoRenderDependency<'a>> IntoRenderDependencies<'a> for T {
    fn into_render_dependencies(self) -> impl Iterator<Item = RenderDependency> {
        vec![self.into_render_dependency()].into_iter()
    }
}

macro_rules! impl_into_render_resource_ids {
    ($(($T: ident, $t: ident)),*) => {
        impl <'a, $($T: IntoRenderDependency<'a>),*> IntoRenderDependencies<'a> for ($($T,)*) {
            fn into_render_dependencies(self) -> impl Iterator<Item = RenderDependency> {
                let ($($t,)*) = self;
                vec![$($t.into_render_dependency()),*].into_iter()
            }
        }
    };
}

all_tuples!(impl_into_render_resource_ids, 0, 16, T, t);