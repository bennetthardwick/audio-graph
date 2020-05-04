use super::{Node, NodeId, Route, RouteGraph};
use sample::Sample;
use std::hash::Hash;

pub struct RouteGraphBuilder<Id, S, R, C, G>
where
    Id: NodeId<G> + Eq,
    S: Sample + Default,
    R: Route<S, C>,
{
    nodes: Vec<Node<Id, S, R, C, G>>,
    buffer_size: usize,
}

impl<Id, S, R, C, G> RouteGraphBuilder<Id, S, R, C, G>
where
    Id: Eq + Hash + NodeId<G>,
    S: Sample + Default,
    R: Route<S, C>,
{
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            buffer_size: 1024,
        }
    }

    pub fn with_node(mut self, node: Node<Id, S, R, C, G>) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn with_buffer_size(mut self, buffer_size: usize) -> Self {
        self.buffer_size = buffer_size;
        self
    }

    pub fn with_nodes(mut self, mut nodes: Vec<Node<Id, S, R, C, G>>) -> Self {
        self.nodes.extend(nodes.drain(..));
        self
    }

    pub fn build(self) -> RouteGraph<Id, S, R, C, G> {
        RouteGraph::with_nodes(self.nodes, self.buffer_size)
    }
}

impl<Id, S, R, C, G> Default for RouteGraphBuilder<Id, S, R, C, G>
where
    Id: Eq + Hash + NodeId<G>,
    S: Sample + Default,
    R: Route<S, C>,
{
    fn default() -> Self {
        RouteGraphBuilder::new()
    }
}
