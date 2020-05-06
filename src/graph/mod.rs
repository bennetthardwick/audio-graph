extern crate bufferpool;

pub mod builder;
pub mod node;

pub use builder::*;
pub use node::*;

use crate::route::Route;
use nano_arena::{Arena, ArenaAccess, Idx};
use sample::Sample;
use std::collections::{HashMap, HashSet};

use bufferpool::{BufferPool, BufferPoolBuilder, BufferPoolReference};

pub struct RouteGraph<S: Sample + Default, R: Route<S, C>, C> {
    ordering: Vec<Idx>,

    temp_ordering: Vec<Idx>,

    stack: Vec<Idx>,

    visited: Vec<bool>,

    temp: Vec<BufferPoolReference<S>>,

    arena: Arena<Node<S, R, C>>,

    max_channels: usize,

    pool: BufferPool<S>,

    sorted: bool,
}

// Implement Send and Sync if all the routes are Send.
// The problem is buffer pool - which has a bunch of mutable
// references and such. But RouteGraph should be fine to send
// between threads so long as it's routes are safe to send
// between threads.
unsafe impl<S, R, C> Send for RouteGraph<S, R, C>
where
    S: Sample + Default,
    R: Route<S, C> + Send,
{
}

unsafe impl<S, R, C> Sync for RouteGraph<S, R, C>
where
    S: Sample + Default,
    R: Route<S, C> + Send,
{
}

impl<S, R, C> Default for RouteGraph<S, R, C>
where
    S: Sample + Default,
    R: Route<S, C>,
{
    fn default() -> Self {
        Self::new()
    }
}

struct IterMutConnections<'a, S: Sample + Default, R: Route<S, C>, C> {
    current: Idx,
    arena: &'a mut Arena<Node<S, R, C>>,
}

impl<S, R, C> RouteGraph<S, R, C>
where
    S: Sample + Default,
    R: Route<S, C>,
{
    // pub fn with_nodes(nodes: Vec<Node<S, R, C>>, buffer_size: usize) -> Self {
    //     // Increment the ordering, visited and stack so they can be used
    //     // for searching without having to alloc memeory
    //     let routes = nodes
    //         .iter()
    //         .map(|node| node.id.clone())
    //         .collect::<Vec<Idx>>();

    //     let max_channels = nodes.iter().fold(0, |a, b| a.max(b.channels));

    //     let capacity = nodes.len();

    //     let mut graph = RouteGraph {
    //         ordering: routes.clone(),
    //         routes,
    //         temp_ordering: Vec::with_capacity(capacity),
    //         visited: HashSet::with_capacity(capacity),
    //         stack: Vec::with_capacity(capacity),
    //         temp: Vec::with_capacity(max_channels),
    //         max_channels,
    //         pool: BufferPoolBuilder::new()
    //             .with_capacity(0)
    //             .with_buffer_size(0)
    //             .build(),
    //         // __context: std::marker::PhantomData::default(),
    //         sorted: false,
    //     };

    //     let buffers = graph.count_required_temp_buffers();

    //     graph.pool = BufferPoolBuilder::new()
    //         .with_capacity(buffers + max_channels)
    //         .with_buffer_size(buffer_size)
    //         .build();

    //     graph
    // }

    pub fn process(&mut self, frames: usize, context: &mut C) {
        if self.buffer_size() < frames {
            self.set_buffer_size(frames);
        }

        let temp = &mut self.temp;
        let arena = &mut self.arena;

        let pool = &mut self.pool;

        for _ in 0..self.max_channels {
            temp.push(pool.get_space().unwrap());
        }

        let len = arena.len();

        for i in 0..len {
            if let Some((current, mut rest)) = arena
                .get_idx_at_index(i)
                .and_then(|idx| arena.split_at(idx))
            {
                let buffers = &current.buffers;
                let node_route = &mut current.route;
                let connections = &current.connections;

                node_route.process(buffers, temp, frames, context);

                for send in connections {
                    if let Some(out_route) = rest.get_mut(&send.id) {
                        if out_route.buffers.len() < out_route.channels {
                            for _ in 0..(out_route.channels - out_route.buffers.len()) {
                                out_route.buffers.push(pool.get_cleared_space().unwrap());
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

                current.buffers.drain(..).for_each(drop);
            }
        }

        temp.drain(..).for_each(drop);
    }

    /// Change the graph buffer size
    ///
    /// # Panics
    /// If any of the internal buffers have been borrowed
    pub fn set_buffer_size(&mut self, buffer: usize) {
        self.pool.change_buffer_size(buffer);
    }

    pub fn buffer_size(&self) -> usize {
        self.pool.get_buffer_size()
    }

    pub fn is_sorted(&self) -> bool {
        self.sorted
    }

    // TODO: Add better new Method
    pub fn new() -> Self {
        RouteGraph {
            ordering: vec![],
            temp_ordering: vec![],

            stack: vec![],
            visited: vec![],
            temp: vec![],
            arena: Arena::new(),
            pool: BufferPool::default(),

            max_channels: 0,
            sorted: true,
        }
    }

    fn count_required_temp_buffers(&mut self) -> usize {
        let mut count: usize = 0;
        let mut max: usize = 0;

        let ordering = &self.ordering;
        let arena = &mut self.arena;

        for route in ordering.iter() {
            if let Some(current) = arena.get(route) {
                let connections = &current.connections;

                count += current.channels;

                for send in connections {
                    if let Some(out_route) = arena.get(&send.id) {
                        count += out_route.channels;
                    }
                }

                max = max.max(count);
                count -= current.channels.min(count);
            }
        }

        max
    }

    pub fn topographic_sort(&mut self) {
        // Set all visited elements to false
        let visited = &mut (self.visited);
        visited.truncate(0);
        visited.resize(self.arena.len(), false);

        let ordering = &mut (self.ordering);
        ordering.truncate(self.arena.len());

        let mut index = self.arena.len();

        for (id, _) in self.arena.entries() {
            if !visited[id.value().unwrap()] {
                // Rust Vecs don't dealloc when you resize them, so this
                // is safe to do. Just remember to resize at the end so pushing
                // will alloc more!

                self.stack.truncate(0);
                self.temp_ordering.truncate(0);

                let stack = &mut (self.stack);
                let temp_ordering = &mut (self.temp_ordering);

                stack.push(id.clone());

                while let Some(current) = stack.pop() {
                    if !visited[current.value().unwrap()] {
                        if let Some(node) = self.arena.get(&current) {
                            for out in node.connections.iter() {
                                if !visited[out.id.value().unwrap()] {
                                    stack.push(out.id.clone());
                                }
                            }
                        }

                        visited[current.value().unwrap()] = true;
                        temp_ordering.push(current);
                    }
                }

                for node in temp_ordering.drain(..).rev() {
                    index -= 1;
                    ordering[index] = node;
                }
            }
        }

        assert_eq!(ordering.len(), self.arena.len());

        self.sorted = true;
    }

    pub fn silence_all_buffers(&mut self) {
        self.pool.clear();
    }

    pub fn len(&self) -> usize {
        self.arena.len()
    }

    // Set the volume / amount of a particular route
    pub fn set_route_amount(&mut self, source: Idx, target: Idx, amount: S) {
        self.with_node_connections(source, |connections| {
            if let Some(position) = connections.iter().position(|c| &c.id == &target) {
                if amount == S::equilibrium() {
                    connections.swap_remove(position);
                } else {
                    connections.get_mut(position).unwrap().amount = amount;
                }
            } else {
                if amount != S::equilibrium() {
                    connections.push(Connection::new(target.clone(), amount))
                }
            }
        });
    }

    pub fn with_node<T, F: FnOnce(&mut Node<S, R, C>) -> T>(
        &mut self,
        id: Idx,
        func: F,
    ) -> Option<T> {
        self.arena.get_mut(id).map(func)
    }

    pub fn with_node_connections<T, F: FnOnce(&mut Vec<Connection<S>>) -> T>(
        &mut self,
        id: Idx,
        func: F,
    ) -> Option<T> {
        self.with_node(id, |node| func(&mut node.connections))
    }

    pub fn remove_node(&mut self, id: Idx) {
        self.arena.swap_remove(&id);
        for node in self.arena.iter_mut() {
            node.connections.retain(|connection| &connection.id != &id);
        }
        self.sorted = false;
    }

    pub fn add_node_with_idx<F: FnMut(Idx) -> Node<S, R, C>>(&mut self, mut func: F) -> Idx {
        let id = self.arena.alloc_with_idx(|id| func(id));

        self.pool.reserve(1);
        self.visited.reserve(1);

        id

        // self.ordering.id
    }

    // pub fn add_node(&mut self, route: Node<S, R, C>) {
    //     let id = &route.id;

    //     self.routes.push(id.clone());

    //     // Increment the ordering, visited and stack so they can be used
    //     // for searching without having to alloc memeory
    //     self.ordering.push(id.clone());
    //     self.temp_ordering.reserve(1);
    //     self.visited.reserve(1);
    //     self.stack.reserve(1);
    //     self.sorted = false;
    //     self.pool.reserve(1);

    //     self.max_channels = self.max_channels.max(route.channels);

    //     if route.channels > self.temp.capacity() {
    //         self.temp.reserve(route.channels - self.temp.capacity());
    //     }

    //     self.route_map.insert(id.clone(), route);
    // }

    pub fn has_cycles(&mut self) -> bool {
        let visited = &mut (self.visited);

        visited.truncate(0);
        visited.resize(self.arena.len(), false);

        for (id, route) in self.arena.entries() {
            visited[id.value().unwrap()] = true;

            for out in &route.connections {
                if visited[out.id.value().unwrap()] {
                    return true;
                }
            }
        }

        self.sorted = true;

        false
    }
}

#[cfg(test)]
mod tests {
    use alloc_counter::{deny_alloc, AllocCounterSystem};

    #[global_allocator]
    static A: AllocCounterSystem = AllocCounterSystem;

    use super::*;
    use crate::route::Route;
    use bufferpool::BufferPoolReference;
    use std::any::Any;
    use volatile_unique_id::*;

    struct TestRoute;

    #[derive(Clone, Debug, Eq, PartialEq, Hash)]
    struct Id {
        id: volatile_unique_id::Id,
    }

    impl NodeId<Generator> for Id {
        fn generate_node_id(generator: &mut Generator) -> Self {
            Id {
                id: generator.generate(),
            }
        }
    }

    trait AnyRoute<S: sample::Sample>: Route<S, C> {
        fn as_any(&self) -> &dyn Any;
    }

    type S = f32;
    type C = ();
    type R = Box<dyn AnyRoute<S>>;
    type N = Node<Id, S, R, C, Generator>;

    impl Route<S, C> for TestRoute {
        fn process(
            &mut self,
            input: &[BufferPoolReference<S>],
            output: &mut [BufferPoolReference<S>],
            _frames: usize,
            _context: &mut C,
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
            _context: &mut C,
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
            _context: &mut C,
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
            _context: &mut C,
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
            context: &mut C,
        ) {
            (**self).process(input, output, frames, context);
        }
    }

    fn create_node(id: Id, mut connections: Vec<Id>) -> N {
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
        let mut generator = GeneratorBuilder::new().build();

        let source_id = Id::generate_node_id(&mut generator);
        let a_id = Id::generate_node_id(&mut generator);
        let b_id = Id::generate_node_id(&mut generator);
        let c_id = Id::generate_node_id(&mut generator);
        let output_id = Id::generate_node_id(&mut generator);

        let source: N = Node::with_id(
            source_id.clone(),
            1,
            Box::new(InputRoute {
                input: vec![0.5; 32],
            }),
            vec![
                Connection::new(a_id.clone(), 1.),
                Connection::new(b_id.clone(), 0.5),
                Connection::new(c_id.clone(), 0.5),
            ],
        );

        let output: N = Node::with_id(
            output_id.clone(),
            1,
            Box::new(OutputRoute {
                output: vec![0.; 32],
            }),
            vec![],
        );

        let a = create_node(a_id.clone(), vec![output_id.clone()]);
        let b = create_node(b_id.clone(), vec![output_id.clone()]);
        let c = create_node(c_id.clone(), vec![output_id.clone()]);

        let mut graph = RouteGraphBuilder::new()
            .with_buffer_size(32)
            .with_nodes(vec![source, a, b, c, output])
            .build();

        assert_eq!(graph.has_cycles(), false);

        let mut c = ();

        deny_alloc(|| {
            graph.process(32, &mut c);
        });

        let route = &graph.route_map.get(&output_id).unwrap().route;

        assert_eq!(
            route.as_any().downcast_ref::<OutputRoute>().unwrap().output,
            vec![1.; 32]
        );
    }

    #[test]
    fn test_signal_flow() {
        let mut generator = GeneratorBuilder::new().build();
        let source_id = Id::generate_node_id(&mut generator);
        let a_id = Id::generate_node_id(&mut generator);
        let b_id = Id::generate_node_id(&mut generator);
        let output_id = Id::generate_node_id(&mut generator);

        let source: N = Node::with_id(
            source_id,
            1,
            Box::new(InputRoute {
                input: vec![1.; 32],
            }),
            vec![Connection::new(a_id.clone(), 1.)],
        );
        let output: N = Node::with_id(
            output_id.clone(),
            1,
            Box::new(OutputRoute {
                output: vec![0.; 32],
            }),
            vec![],
        );

        let a = create_node(a_id.clone(), vec![b_id.clone()]);
        let b = create_node(b_id.clone(), vec![output_id.clone()]);

        let mut graph: RouteGraph<Id, S, R, C, Generator> =
            RouteGraphBuilder::new().with_buffer_size(32).build();
        graph.add_node(source);
        graph.add_node(a);
        graph.add_node(b);
        graph.add_node(output);

        assert_eq!(graph.has_cycles(), false);

        let mut c = ();

        deny_alloc(|| {
            graph.process(32, &mut c);
        });

        let route = &graph.route_map.get(&output_id).unwrap().route;

        assert_eq!(
            route.as_any().downcast_ref::<OutputRoute>().unwrap().output,
            vec![1.; 32]
        );
    }

    #[test]
    fn test_signal_flow_counting() {
        let mut generator = GeneratorBuilder::new().build();
        let source_id = Id::generate_node_id(&mut generator);
        let output_id = Id::generate_node_id(&mut generator);

        let source: N = Node::with_id(
            source_id.clone(),
            1,
            Box::new(CountingNode { current: 0 }),
            vec![Connection::new(output_id.clone(), 1.)],
        );
        let output: N = Node::with_id(
            output_id.clone(),
            1,
            Box::new(OutputRoute {
                output: vec![0.; 1024],
            }),
            vec![],
        );

        let mut graph = RouteGraphBuilder::new().with_buffer_size(1024).build();
        graph.add_node(source);
        graph.add_node(output);

        let mut c = ();

        deny_alloc(|| {
            graph.process(1024, &mut c);
        });

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
        let mut generator = GeneratorBuilder::new().build();
        let a_id = Id::generate_node_id(&mut generator);
        let b_id = Id::generate_node_id(&mut generator);

        let a = create_node(a_id.clone(), vec![b_id.clone()]);
        let b = create_node(b_id.clone(), vec![]);

        let mut graph = RouteGraph::new();
        graph.add_node(b);
        graph.add_node(a);

        assert_eq!(graph.routes, vec![b_id.clone(), a_id.clone()]);
        assert_eq!(graph.has_cycles(), true);

        graph.topographic_sort();

        assert_eq!(graph.routes, vec![a_id, b_id]);
        assert_eq!(graph.has_cycles(), false);
    }

    #[test]
    fn test_long_line_topo_sort() {
        let mut generator = GeneratorBuilder::new().build();

        let a_id = Id::generate_node_id(&mut generator);
        let b_id = Id::generate_node_id(&mut generator);
        let c_id = Id::generate_node_id(&mut generator);
        let d_id = Id::generate_node_id(&mut generator);
        let e_id = Id::generate_node_id(&mut generator);
        let f_id = Id::generate_node_id(&mut generator);

        let ids = vec![
            a_id.clone(),
            b_id.clone(),
            c_id.clone(),
            d_id.clone(),
            e_id.clone(),
            f_id.clone(),
        ];

        let a = create_node(a_id, vec![b_id.clone()]);
        let b = create_node(b_id, vec![c_id.clone()]);
        let c = create_node(c_id, vec![d_id.clone()]);
        let d = create_node(d_id, vec![e_id.clone()]);
        let e = create_node(e_id, vec![f_id.clone()]);
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
        let mut generator = GeneratorBuilder::new().build();

        let a_id = Id::generate_node_id(&mut generator);
        let b_id = Id::generate_node_id(&mut generator);
        let c_id = Id::generate_node_id(&mut generator);
        let d_id = Id::generate_node_id(&mut generator);
        let e_id = Id::generate_node_id(&mut generator);
        let f_id = Id::generate_node_id(&mut generator);

        let ids = vec![
            a_id.clone(),
            b_id.clone(),
            c_id.clone(),
            d_id.clone(),
            e_id.clone(),
            f_id.clone(),
        ];

        for (i, a) in ids.iter().enumerate() {
            for (j, b) in ids.iter().enumerate() {
                if i == j {
                    continue;
                }

                assert_ne!(*a, *b);
            }
        }

        let a = create_node(a_id, vec![b_id.clone(), d_id.clone()]);
        let b = create_node(b_id, vec![]);
        let c = create_node(c_id, vec![f_id.clone()]);
        let d = create_node(d_id, vec![e_id.clone(), f_id.clone()]);
        let e = create_node(e_id, vec![f_id.clone()]);
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
