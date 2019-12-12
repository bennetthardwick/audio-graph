use crate::graph::node::NodeId;
use log::{error, warn};
use sample::Sample;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

use crate::graph::node::Node;
use bufferpool::{BufferPool, BufferPoolBuilder, BufferPoolReference};

pub struct RouteGraphBuilder<RouteId: NodeId + Debug + Eq, S: Sample + Default> {
    nodes: Vec<Node<RouteId, S>>,
    buffer_size: usize,
}

impl<RouteId, S: Sample + Default> RouteGraphBuilder<RouteId, S>
where
    RouteId: Eq + Hash + Copy + NodeId + Debug,
{
    pub fn new() -> RouteGraphBuilder<RouteId, S> {
        RouteGraphBuilder {
            nodes: vec![],
            buffer_size: 1024,
        }
    }

    pub fn with_node(mut self, node: Node<RouteId, S>) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn with_buffer_size(mut self, buffer_size: usize) -> Self {
        self.buffer_size = buffer_size;
        self
    }

    pub fn with_nodes(mut self, mut nodes: Vec<Node<RouteId, S>>) -> Self {
        self.nodes.extend(nodes.drain(..));
        self
    }

    pub fn build(self) -> RouteGraph<RouteId, S> {
        RouteGraph::with_nodes(self.nodes, self.buffer_size)
    }
}

pub struct RouteGraph<RouteId: NodeId, S: Sample + Default> {
    ordering: Vec<RouteId>,
    temp_ordering: Vec<RouteId>,
    stack: Vec<RouteId>,
    visited: HashSet<RouteId>,

    temp: Vec<BufferPoolReference<S>>,

    routes: Vec<RouteId>,
    route_map: HashMap<RouteId, Node<RouteId, S>>,

    pool: BufferPool<S>,

    sorted: bool,
}

impl<RouteId, S: Sample + Default> RouteGraph<RouteId, S>
where
    RouteId: Eq + Hash + Copy + NodeId + Debug,
{
    pub fn with_nodes(
        mut nodes: Vec<Node<RouteId, S>>,
        buffer_size: usize,
    ) -> RouteGraph<RouteId, S> {
        // Increment the ordering, visited and stack so they can be used
        // for searching without having to alloc memeory

        let routes = nodes.iter().map(|node| node.id).collect::<Vec<RouteId>>();

        let mut graph = RouteGraph {
            ordering: routes.clone(),
            routes,
            temp_ordering: Vec::with_capacity(nodes.len()),
            visited: HashSet::with_capacity(nodes.len()),
            stack: Vec::with_capacity(nodes.len()),
            temp: vec![],
            route_map: nodes.drain(..).map(|node| (node.id, node)).collect(),
            pool: BufferPool::default(),
            sorted: false,
        };

        let buffers = graph.count_required_temp_buffers();

        graph.pool = BufferPoolBuilder::new()
            .with_capacity(buffers)
            .with_buffer_size(buffer_size)
            .build();

        graph
    }

    // TODO: Add New Method
    pub fn new() -> RouteGraph<RouteId, S> {
        RouteGraph {
            ordering: vec![],
            temp_ordering: vec![],
            stack: vec![],
            visited: HashSet::new(),

            temp: vec![],

            routes: vec![],
            route_map: HashMap::new(),

            pool: BufferPool::default(),

            sorted: false,
        }
    }

    fn count_required_temp_buffers(&mut self) -> usize {
        let mut count: usize = 0;
        let mut max: usize = 0;

        let routes = &self.routes;
        let route_map = &mut self.route_map;

        for route in routes.iter() {
            if let Some(current) = route_map.remove(route) {
                let connections = &current.connections;

                for send in connections {
                    if let Some(out_route) = route_map.get(&send.id) {
                        count += out_route.channels;
                    }
                }

                max = max.max(count);
                count -= current.channels;
            }
        }

        max
    }

    pub fn topographic_sort(&mut self) {
        // Set all visited elements to false
        let visited = &mut (self.visited);
        visited.clear();

        let ordering = &mut (self.ordering);

        let mut index = self.routes.len();

        for route in self.routes.iter() {
            if !visited.contains(route) {
                // Rust Vecs don't dealloc when you resize them, so this
                // is safe to do. Just remember to resize at the end so pushing
                // will alloc more!

                unsafe {
                    self.stack.set_len(0);
                    self.temp_ordering.set_len(0);
                }

                let stack = &mut (self.stack);
                let temp_ordering = &mut (self.temp_ordering);

                stack.push(*route);

                while let Some(current) = stack.pop() {
                    if !visited.contains(&current) {
                        if let Some(node) = self.route_map.get(&current) {
                            for out in node.connections.iter() {
                                if !visited.contains(&out.id) {
                                    stack.push(out.id);
                                }
                            }
                        }

                        temp_ordering.push(current);
                        visited.insert(current);
                    }
                }

                for node in temp_ordering.drain(..).rev() {
                    if self.route_map.contains_key(&node) {
                        index -= 1;
                        ordering[index] = node;
                    } else {
                        error!("Node {:?} was targetted, despite not existing in map", node);
                    }
                }
            }
        }

        assert_eq!(ordering.len(), self.routes.len());

        for (route, ordered) in self.routes.iter_mut().zip(ordering.iter()) {
            *route = *ordered;
        }

        self.sorted = true;
    }

    pub fn process(&mut self, frames: usize) {
        let temp = &mut self.temp;
        let routes = &self.routes;
        let route_map = &mut self.route_map;

        for route in routes.iter() {
            if let Some(mut current) = route_map.remove(route) {
                let buffers = &current.buffers;
                let node_route = &mut current.route;
                let connections = &current.connections;

                node_route.process(buffers, temp, frames);

                for send in connections {
                    if let Some(out_route) = route_map.get_mut(&send.id) {
                        for _ in 0..out_route.channels {
                            out_route
                                .buffers
                                .push(self.pool.get_cleared_space().unwrap());
                        }

                        for (output_vector, input_vector) in
                            out_route.buffers.iter_mut().zip(temp.iter())
                        {
                            for (output, input) in output_vector
                                .as_mut()
                                .iter_mut()
                                .zip(input_vector.as_ref().iter())
                            {
                                output.add_amp(
                                    input
                                        .mul_amp(send.amount.to_float_sample())
                                        .to_signed_sample(),
                                );
                            }
                        }
                    }
                }

                // Remove the buffers that were left over from last time.
                current.buffers.drain(..).for_each(drop);

                route_map.insert(*route, current);
            }
        }
    }

    pub fn silence_all_buffers(&mut self) {
        self.pool.clear();
    }

    pub fn add_route(&mut self, route: Node<RouteId, S>) {
        let id = route.id;

        self.routes.push(id);

        // Increment the ordering, visited and stack so they can be used
        // for searching without having to alloc memeory
        self.ordering.push(id);
        self.temp_ordering.reserve(1);
        self.visited.reserve(1);
        self.stack.reserve(1);
        self.sorted = false;
        self.pool.reserve(1);

        self.route_map.insert(id, route);
    }

    pub fn verify(&mut self) -> bool {
        let visited = &mut (self.visited);
        visited.clear();

        for route_id in self.routes.iter() {
            visited.insert(*route_id);

            if let Some(route) = self.route_map.get(route_id) {
                for out_route in &route.connections {
                    if visited.contains(&out_route.id) {
                        warn!(
                            "Route {:?} has an out route that references visited route {:?}",
                            route_id, out_route.id
                        );
                        return false;
                    }
                }
            }
        }

        self.sorted = true;

        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::route::Route;
    use bufferpool::BufferPoolReference;
    use std::any::Any;
    use uuid::Uuid;

    #[derive(Debug)]
    struct TestRoute;

    #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
    struct Id {
        uuid: Uuid,
    }

    impl Id {
        fn from_str(string: &str) -> Self {
            Id {
                uuid: Uuid::parse_str(string).unwrap(),
            }
        }
    }

    impl NodeId for Id {
        fn generate_node_id() -> Self {
            Id {
                uuid: Uuid::new_v4(),
            }
        }
    }

    type S = f32;

    impl Route<S> for TestRoute {
        fn process(
            &mut self,
            input: &[BufferPoolReference<S>],
            output: &mut [BufferPoolReference<S>],
            _frames: usize,
        ) {
            for (a, b) in output.iter_mut().zip(input.iter()) {
                for (output, input) in a.as_mut().iter_mut().zip(b.as_ref().iter()) {
                    *output = *input;
                }
            }
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[derive(Debug)]
    struct InputRoute {
        input: Vec<S>,
    }

    impl Route<S> for InputRoute {
        fn process(
            &mut self,
            _input: &[BufferPoolReference<S>],
            output: &mut [BufferPoolReference<S>],
            _frames: usize,
        ) {
            for stream in output.iter_mut() {
                for (output, input) in stream.as_mut().iter_mut().zip(self.input.iter()) {
                    *output = *input;
                }
            }
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[derive(Debug)]
    struct OutputRoute {
        output: Vec<S>,
    }

    impl Route<S> for OutputRoute {
        fn process(
            &mut self,
            input: &[BufferPoolReference<S>],
            _output: &mut [BufferPoolReference<S>],
            _frames: usize,
        ) {
            for stream in input.iter() {
                for (output, input) in self.output.iter_mut().zip(stream.as_ref().iter()) {
                    *output = *input;
                }
            }
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    fn create_node(id: Id, mut connections: Vec<Id>) -> Node<Id, f32> {
        Node::with_id(
            id,
            1,
            Box::new(TestRoute),
            connections
                .drain(..)
                .map(|id| Connection::new(id, 1.))
                .collect::<Vec<Connection<Id, S>>>(),
        )
    }

    #[test]
    fn test_signal_flow() {
        let source_id = Id::generate_node_id();
        let a_id = Id::generate_node_id();
        let b_id = Id::generate_node_id();
        let output_id = Id::generate_node_id();

        let source = Node::with_id(
            source_id,
            1,
            Box::new(InputRoute {
                input: vec![1.; 1024],
            }),
            vec![Connection::new(a_id, 1.)],
        );
        let output = Node::with_id(
            output_id,
            1,
            Box::new(OutputRoute {
                output: vec![0.; 1024],
            }),
            vec![],
        );

        let a = create_node(a_id, vec![b_id]);
        let b = create_node(b_id, vec![output_id]);

        let mut graph = RouteGraph::new();
        graph.add_route(source);
        graph.add_route(a);
        graph.add_route(b);
        graph.add_route(output);

        assert_eq!(graph.verify(), true);

        graph.process(1024);

        let route = &graph.route_map.get(&output_id).unwrap().route;

        assert_eq!(
            route.as_any().downcast_ref::<OutputRoute>().unwrap().output,
            vec![1.; 1024]
        );
    }

    #[test]
    fn test_simple_topo_sort() {
        let a_id = Id::generate_node_id();
        let b_id = Id::generate_node_id();

        let a = create_node(a_id, vec![b_id]);
        let b = create_node(b_id, vec![]);

        let mut graph = RouteGraph::new();
        graph.add_route(b);
        graph.add_route(a);

        assert_eq!(graph.routes, vec![b_id, a_id]);
        assert_eq!(graph.verify(), false);

        graph.topographic_sort();

        assert_eq!(graph.routes, vec![a_id, b_id]);
        assert_eq!(graph.verify(), true);
    }

    #[test]
    fn test_long_line_topo_sort() {
        let a_id = Id::from_str("a0000000-0000-0000-0000-000000000000");
        let b_id = Id::from_str("b0000000-0000-0000-0000-000000000000");
        let c_id = Id::from_str("c0000000-0000-0000-0000-000000000000");
        let d_id = Id::from_str("d0000000-0000-0000-0000-000000000000");
        let e_id = Id::from_str("e0000000-0000-0000-0000-000000000000");
        let f_id = Id::from_str("f0000000-0000-0000-0000-000000000000");

        let ids = vec![a_id, b_id, c_id, d_id, e_id, f_id];

        let a = create_node(a_id, vec![b_id]);
        let b = create_node(b_id, vec![c_id]);
        let c = create_node(c_id, vec![d_id]);
        let d = create_node(d_id, vec![e_id]);
        let e = create_node(e_id, vec![f_id]);
        let f = create_node(f_id, vec![]);

        let mut graph = RouteGraph::new();
        graph.add_route(b);
        graph.add_route(d);
        graph.add_route(e);
        graph.add_route(f);
        graph.add_route(c);
        graph.add_route(a);

        assert_eq!(graph.verify(), false);

        graph.topographic_sort();

        assert_eq!(graph.routes, ids);
        assert_eq!(graph.verify(), true);
    }

    #[test]
    fn test_crazy_topo_sort() {
        let a_id = Id::from_str("a0000000-0000-0000-0000-000000000000");
        let b_id = Id::from_str("b0000000-0000-0000-0000-000000000000");
        let c_id = Id::from_str("c0000000-0000-0000-0000-000000000000");
        let d_id = Id::from_str("d0000000-0000-0000-0000-000000000000");
        let e_id = Id::from_str("e0000000-0000-0000-0000-000000000000");
        let f_id = Id::from_str("f0000000-0000-0000-0000-000000000000");

        let ids = vec![a_id, b_id, c_id, d_id, e_id, f_id];

        for (i, a) in ids.iter().enumerate() {
            for (j, b) in ids.iter().enumerate() {
                if i == j {
                    continue;
                }

                assert_ne!(*a, *b);
            }
        }

        let a = create_node(a_id, vec![b_id, d_id]);
        let b = create_node(b_id, vec![]);
        let c = create_node(c_id, vec![f_id]);
        let d = create_node(d_id, vec![e_id, f_id]);
        let e = create_node(e_id, vec![f_id]);
        let f = create_node(f_id, vec![]);

        let mut graph = RouteGraph::new();
        graph.add_route(f);
        graph.add_route(d);
        graph.add_route(b);
        graph.add_route(e);
        graph.add_route(a);
        graph.add_route(c);

        assert_eq!(graph.verify(), false);

        graph.topographic_sort();

        assert_eq!(graph.verify(), true);
    }
}
