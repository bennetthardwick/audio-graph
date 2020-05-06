use super::{Node, Route, RouteGraph};
use sample::Sample;

pub struct RouteGraphBuilder<S, R, C>
where
    S: Sample + Default,
    R: Route<S, C>,
{
    nodes: Vec<Node<S, R, C>>,
    buffer_size: usize,
}

impl<S, R, C> RouteGraphBuilder<S, R, C>
where
    S: Sample + Default,
    R: Route<S, C>,
{
    // pub fn new() -> Self {
    //     Self {
    //         nodes: vec![],
    //         buffer_size: 1024,
    //     }
    // }

    // pub fn with_node(mut self, node: Node<S, R, C>) -> Self {
    //     self.nodes.push(node);
    //     self
    // }

    // pub fn with_buffer_size(mut self, buffer_size: usize) -> Self {
    //     self.buffer_size = buffer_size;
    //     self
    // }

    // pub fn with_nodes(mut self, mut nodes: Vec<Node<S, R, C>>) -> Self {
    //     self.nodes.extend(nodes.drain(..));
    //     self
    // }

    // pub fn build(self) -> RouteGraph<S, R, C> {
    //     RouteGraph::with_nodes(self.nodes, self.buffer_size)
    // }
}

// impl<S, R, C> Default for RouteGraphBuilder<S, R, C>
// where
//     S: Sample + Default,
//     R: Route<S, C>,
// {
//     fn default() -> Self {
//         RouteGraphBuilder::new()
//     }
// }
