use crate::render_graph::{RenderLabel, RenderSubGraph};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, RenderSubGraph)]
pub struct DefaultCompositorGraph;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, RenderLabel)]
pub struct CompositorNode;

// TODO:
// - setup compositor graph structure, and defer to view render graph
// - extraction and such
// - module structure. This all probably shouldn't still live in `Camera`.
// - move `ComputedCameraValues` around. merge with Frustum?
// - investigate utility camera query data
// - fix event dispatch
// - fix relationship hooks
