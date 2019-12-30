use super::{Node, NodeId, Route, RouteGraph};
use sample::Sample;
use std::hash::Hash;

pub struct RouteGraphBuilder<Id, S, R>
where
    Id: NodeId + Eq,
    S: Sample + Default,
    R: Route<S>,
{
    nodes: Vec<Node<Id, S, R>>,
    buffer_size: usize,
}

impl<Id, S, R> RouteGraphBuilder<Id, S, R>
where
    Id: Eq + Hash + Copy + NodeId,
    S: Sample + Default,
    R: Route<S>,
{
    pub fn new() -> RouteGraphBuilder<Id, S, R> {
        RouteGraphBuilder {
            nodes: vec![],
            buffer_size: 1024,
        }
    }

    pub fn with_node(mut self, node: Node<Id, S, R>) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn with_buffer_size(mut self, buffer_size: usize) -> Self {
        self.buffer_size = buffer_size;
        self
    }

    pub fn with_nodes(mut self, mut nodes: Vec<Node<Id, S, R>>) -> Self {
        self.nodes.extend(nodes.drain(..));
        self
    }

    pub fn build(self) -> RouteGraph<Id, S, R> {
        RouteGraph::with_nodes(self.nodes, self.buffer_size)
    }
}

impl<Id, S, R> Default for RouteGraphBuilder<Id, S, R>
where
    Id: Eq + Hash + Copy + NodeId,
    S: Sample + Default,
    R: Route<S>,
{
    fn default() -> RouteGraphBuilder<Id, S, R> {
        RouteGraphBuilder::new()
    }
}
