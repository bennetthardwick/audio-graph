extern crate bufferpool;

pub mod builder;
pub mod node;

pub use builder::*;
pub use node::*;

use crate::route::Route;
use sample::Sample;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use bufferpool::{BufferPool, BufferPoolBuilder, BufferPoolReference};

pub struct RouteGraph<Id: NodeId, S: Sample + Default, R: Route<S, C>, C> {
    ordering: Vec<Id>,
    temp_ordering: Vec<Id>,
    stack: Vec<Id>,
    visited: HashSet<Id>,

    temp: Vec<BufferPoolReference<S>>,

    routes: Vec<Id>,
    route_map: HashMap<Id, Node<Id, S, R, C>>,

    __context: std::marker::PhantomData<*const C>,

    max_channels: usize,

    pool: BufferPool<S>,

    sorted: bool,
}

// Implement Send and Sync if all the routes are Send.
// The problem is buffer pool - which has a bunch of mutable
// references and such. But RouteGraph should be fine to send
// between threads so long as it's routes are safe to send
// between threads.
unsafe impl<Id, S, R, C> Send for RouteGraph<Id, S, R, C>
where
    Id: NodeId,
    S: Sample + Default,
    R: Route<S, C> + Send,
{
}

unsafe impl<Id, S, R, C> Sync for RouteGraph<Id, S, R, C>
where
    Id: NodeId,
    S: Sample + Default,
    R: Route<S, C> + Send,
{
}

impl<Id, S, R, C> Default for RouteGraph<Id, S, R, C>
where
    Id: Eq + Hash + Copy + NodeId,
    S: Sample + Default,
    R: Route<S, C>,
{
    fn default() -> RouteGraph<Id, S, R, C> {
        RouteGraph::new()
    }
}

impl<Id, S, R, C> RouteGraph<Id, S, R, C>
where
    Id: Eq + Hash + Copy + NodeId,
    S: Sample + Default,
    R: Route<S, C>,
{
    pub fn with_nodes(
        nodes: Vec<Node<Id, S, R, C>>,
        buffer_size: usize,
    ) -> RouteGraph<Id, S, R, C> {
        // Increment the ordering, visited and stack so they can be used
        // for searching without having to alloc memeory
        let routes = nodes.iter().map(|node| node.id).collect::<Vec<Id>>();

        let max_channels = nodes.iter().fold(0, |a, b| a.max(b.channels));

        let capacity = nodes.len();

        let route_map = nodes
            .into_iter()
            .map(|node| (node.id, node))
            .collect::<HashMap<_, _>>();

        let mut graph = RouteGraph {
            ordering: routes.clone(),
            routes,
            temp_ordering: Vec::with_capacity(capacity),
            visited: HashSet::with_capacity(capacity),
            stack: Vec::with_capacity(capacity),
            temp: Vec::with_capacity(max_channels),
            max_channels,
            route_map,
            pool: BufferPoolBuilder::new()
                .with_capacity(0)
                .with_buffer_size(0)
                .build(),
            __context: std::marker::PhantomData::default(),
            sorted: false,
        };

        let buffers = graph.count_required_temp_buffers();

        graph.pool = BufferPoolBuilder::new()
            .with_capacity(buffers + max_channels)
            .with_buffer_size(buffer_size)
            .build();

        graph
    }

    pub fn process(&mut self, frames: usize, context: &mut C) {
        let temp = &mut self.temp;
        let routes = &self.routes;
        let route_map = &mut self.route_map;

        for _ in 0..self.max_channels {
            temp.push(self.pool.get_space().unwrap());
        }

        for route in routes.iter() {
            if let Some(mut current) = route_map.remove(route) {
                let buffers = &current.buffers;
                let node_route = &mut current.route;
                let connections = &current.connections;

                node_route.process(buffers, temp, frames, context);

                for send in connections {
                    if let Some(out_route) = route_map.get_mut(&send.id) {
                        if out_route.buffers.len() < out_route.channels {
                            for _ in 0..(out_route.channels - out_route.buffers.len()) {
                                out_route
                                    .buffers
                                    .push(self.pool.get_cleared_space().unwrap());
                            }
                        }

                        for (output_vector, input_vector) in
                            out_route.buffers.iter_mut().zip(temp.iter())
                        {
                            for (output, input) in output_vector
                                .as_mut()
                                .iter_mut()
                                .zip(input_vector.as_ref().iter())
                            {
                                *output = output.add_amp(
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

        temp.drain(..).for_each(drop);
    }

    pub fn buffer_size(&self) -> usize {
        self.pool.get_buffer_size()
    }

    pub fn is_sorted(&self) -> bool {
        self.sorted
    }

    // TODO: Add better new Method
    pub fn new() -> RouteGraph<Id, S, R, C> {
        RouteGraph {
            ordering: vec![],
            temp_ordering: vec![],
            stack: vec![],
            visited: HashSet::new(),

            temp: vec![],

            max_channels: 0,

            routes: vec![],
            route_map: HashMap::new(),

            pool: BufferPool::default(),

            __context: std::marker::PhantomData::default(),

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

                count += current.channels;

                for send in connections {
                    if let Some(out_route) = route_map.get(&send.id) {
                        count += out_route.channels;
                    }
                }

                max = max.max(count);
                count -= current.channels.min(count);

                route_map.insert(*route, current);
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

    pub fn silence_all_buffers(&mut self) {
        self.pool.clear();
    }

    pub fn add_node(&mut self, route: Node<Id, S, R, C>) {
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

        self.max_channels = self.max_channels.max(route.channels);

        if route.channels > self.temp.capacity() {
            self.temp.reserve(route.channels - self.temp.capacity());
        }

        self.route_map.insert(id, route);
    }

    pub fn has_cycles(&mut self) -> bool {
        let visited = &mut (self.visited);
        visited.clear();

        for route_id in self.routes.iter() {
            visited.insert(*route_id);

            if let Some(route) = self.route_map.get(route_id) {
                for out_route in &route.connections {
                    if visited.contains(&out_route.id) {
                        return true;
                    }
                }
            }
        }

        self.sorted = true;

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::Route;
    use bufferpool::BufferPoolReference;
    use std::any::Any;
    use uuid::Uuid;

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

    trait AnyRoute<S: sample::Sample>: Route<S, C> {
        fn as_any(&self) -> &dyn Any;
    }

    type S = f32;
    type C = ();
    type R = Box<dyn AnyRoute<S>>;
    type N = Node<Id, S, R, C>;

    impl Route<S, C> for TestRoute {
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
    }

    impl AnyRoute<S> for TestRoute {
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    struct InputRoute {
        input: Vec<S>,
    }

    impl Route<S, C> for InputRoute {
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
    }

    impl AnyRoute<S> for InputRoute {
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    struct OutputRoute {
        output: Vec<S>,
    }

    impl Route<S, C> for OutputRoute {
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
    }

    impl AnyRoute<S> for OutputRoute {
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    struct CountingNode {
        current: usize,
    }

    impl Route<S, C> for CountingNode {
        fn process(
            &mut self,
            _input: &[BufferPoolReference<S>],
            output: &mut [BufferPoolReference<S>],
            _frames: usize,
        ) {
            for sample in output[0].as_mut().iter_mut() {
                *sample = self.current as f32;
                self.current += 1;
            }
        }
    }

    impl AnyRoute<S> for CountingNode {
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    impl AnyRoute<S> for Box<dyn AnyRoute<S>> {
        fn as_any(&self) -> &dyn Any {
            (**self).as_any()
        }
    }

    impl Route<S, C> for Box<dyn AnyRoute<S>> {
        fn process(
            &mut self,
            input: &[BufferPoolReference<S>],
            output: &mut [BufferPoolReference<S>],
            frames: usize,
        ) {
            (**self).process(input, output, frames);
        }
    }

    fn create_node(id: Id, mut connections: Vec<Id>) -> Node<Id, f32, R, C> {
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
    fn test_multiple_outs_signal_flow() {
        let source_id = Id::generate_node_id();
        let a_id = Id::generate_node_id();
        let b_id = Id::generate_node_id();
        let c_id = Id::generate_node_id();
        let output_id = Id::generate_node_id();

        let source: N = Node::with_id(
            source_id,
            1,
            Box::new(InputRoute {
                input: vec![0.5; 32],
            }),
            vec![
                Connection::new(a_id, 1.),
                Connection::new(b_id, 0.5),
                Connection::new(c_id, 0.5),
            ],
        );

        let output: N = Node::with_id(
            output_id,
            1,
            Box::new(OutputRoute {
                output: vec![0.; 32],
            }),
            vec![],
        );

        let a = create_node(a_id, vec![output_id]);
        let b = create_node(b_id, vec![output_id]);
        let c = create_node(c_id, vec![output_id]);

        let mut graph = RouteGraphBuilder::new()
            .with_buffer_size(32)
            .with_nodes(vec![source, a, b, c, output])
            .build();

        assert_eq!(graph.has_cycles(), false);

        graph.process(32);

        let route = &graph.route_map.get(&output_id).unwrap().route;

        assert_eq!(
            route.as_any().downcast_ref::<OutputRoute>().unwrap().output,
            vec![1.; 32]
        );
    }

    #[test]
    fn test_signal_flow() {
        let source_id = Id::generate_node_id();
        let a_id = Id::generate_node_id();
        let b_id = Id::generate_node_id();
        let output_id = Id::generate_node_id();

        let source: Node<Id, S, R> = Node::with_id(
            source_id,
            1,
            Box::new(InputRoute {
                input: vec![1.; 32],
            }),
            vec![Connection::new(a_id, 1.)],
        );
        let output: Node<Id, S, R> = Node::with_id(
            output_id,
            1,
            Box::new(OutputRoute {
                output: vec![0.; 32],
            }),
            vec![],
        );

        let a = create_node(a_id, vec![b_id]);
        let b = create_node(b_id, vec![output_id]);

        let mut graph: RouteGraph<Id, S, R> = RouteGraphBuilder::new().with_buffer_size(32).build();
        graph.add_node(source);
        graph.add_node(a);
        graph.add_node(b);
        graph.add_node(output);

        assert_eq!(graph.has_cycles(), false);

        graph.process(32);

        let route = &graph.route_map.get(&output_id).unwrap().route;

        assert_eq!(
            route.as_any().downcast_ref::<OutputRoute>().unwrap().output,
            vec![1.; 32]
        );
    }

    #[test]
    fn test_signal_flow_counting() {
        let source_id = Id::generate_node_id();
        let output_id = Id::generate_node_id();

        let source: N = Node::with_id(
            source_id,
            1,
            Box::new(CountingNode { current: 0 }),
            vec![Connection::new(output_id, 1.)],
        );
        let output: N = Node::with_id(
            output_id,
            1,
            Box::new(OutputRoute {
                output: vec![0.; 1024],
            }),
            vec![],
        );

        let mut graph = RouteGraphBuilder::new().with_buffer_size(1024).build();
        graph.add_node(source);
        graph.add_node(output);

        graph.process(1024);

        let mut test: Vec<f32> = vec![0.; 1024];
        for (index, value) in test.iter_mut().enumerate() {
            *value = index as f32;
        }

        let route = &graph.route_map.get(&output_id).unwrap().route;

        assert_eq!(
            route.as_any().downcast_ref::<OutputRoute>().unwrap().output,
            test
        );
    }

    #[test]
    fn test_simple_topo_sort() {
        let a_id = Id::generate_node_id();
        let b_id = Id::generate_node_id();

        let a = create_node(a_id, vec![b_id]);
        let b = create_node(b_id, vec![]);

        let mut graph = RouteGraph::new();
        graph.add_node(b);
        graph.add_node(a);

        assert_eq!(graph.routes, vec![b_id, a_id]);
        assert_eq!(graph.has_cycles(), true);

        graph.topographic_sort();

        assert_eq!(graph.routes, vec![a_id, b_id]);
        assert_eq!(graph.has_cycles(), false);
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
        graph.add_node(b);
        graph.add_node(d);
        graph.add_node(e);
        graph.add_node(f);
        graph.add_node(c);
        graph.add_node(a);

        assert_eq!(graph.has_cycles(), true);

        graph.topographic_sort();

        assert_eq!(graph.routes, ids);
        assert_eq!(graph.has_cycles(), false);
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
        graph.add_node(f);
        graph.add_node(d);
        graph.add_node(b);
        graph.add_node(e);
        graph.add_node(a);
        graph.add_node(c);

        assert_eq!(graph.has_cycles(), true);

        graph.topographic_sort();

        assert_eq!(graph.has_cycles(), false);
    }
}
