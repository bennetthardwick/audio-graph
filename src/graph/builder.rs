use super::{Node, NodeId, Route, RouteGraph};
use sample::Sample;
use std::hash::Hash;

pub struct RouteGraphBuilder<Id, S, R, C>
where
    Id: NodeId + Eq,
    S: Sample + Default,
    R: Route<S, C>,
{
    nodes: Vec<Node<Id, S, R, C>>,
    buffer_size: usize,
    __context: std::marker::PhantomData<*const C>,
}

impl<Id, S, R, C> RouteGraphBuilder<Id, S, R, C>
where
    Id: Eq + Hash + Copy + NodeId,
    S: Sample + Default,
    R: Route<S, C>,
{
    pub fn new() -> RouteGraphBuilder<Id, S, R, C> {
        RouteGraphBuilder {
            nodes: vec![],
            buffer_size: 1024,
            __context: std::marker::PhantomData::default(),
        }
    }

    pub fn with_node(mut self, node: Node<Id, S, R, C>) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn with_buffer_size(mut self, buffer_size: usize) -> Self {
        self.buffer_size = buffer_size;
        self
    }

    pub fn with_nodes(mut self, mut nodes: Vec<Node<Id, S, R, C>>) -> Self {
        self.nodes.extend(nodes.drain(..));
        self
    }

    pub fn build(self) -> RouteGraph<Id, S, R, C> {
        RouteGraph::with_nodes(self.nodes, self.buffer_size)
    }
}

impl<Id, S, R, C> Default for RouteGraphBuilder<Id, S, R, C>
where
    Id: Eq + Hash + Copy + NodeId,
    S: Sample + Default,
    R: Route<S, C>,
{
    fn default() -> RouteGraphBuilder<Id, S, R, C> {
        RouteGraphBuilder::new()
    }
}
