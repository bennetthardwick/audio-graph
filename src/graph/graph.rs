use crate::graph::node::NodeId;
use log::{error, warn};
use sample::Sample;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

use crate::graph::node::Node;
use bufferpool::BufferPool;

pub struct RouteGraph<RouteId: NodeId, S: Sample + Default> {
    ordering: Vec<RouteId>,
    temp_ordering: Vec<RouteId>,
    stack: Vec<RouteId>,
    visited: HashSet<RouteId>,

    routes: Vec<RouteId>,
    route_map: HashMap<RouteId, Node<RouteId, S>>,

    pool: BufferPool<S>,

    sorted: bool,
}

impl<RouteId, S: Sample + Default> RouteGraph<RouteId, S>
where
    RouteId: Eq + Hash + Copy + NodeId + Debug,
{
    // TODO: Add New Method
    pub fn new() -> RouteGraph<RouteId, S> {
        RouteGraph {
            ordering: vec![],
            temp_ordering: vec![],
            stack: vec![],
            visited: HashSet::new(),

            routes: vec![],
            route_map: HashMap::new(),

            pool: BufferPool::default(),

            sorted: false,
        }
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

    type S = f32;

    impl Route<S> for TestRoute {
        fn process(&mut self, input: &[&[S]], output: &[&mut [S]], frames: usize) {
            unimplemented!();
        }
    }

    fn create_node(id: Id, mut connections: Vec<Id>) -> Node<Id, f32> {
        Node::with_id(
            id,
            0,
            Box::new(TestRoute),
            connections
                .drain(..)
                .map(|id| Connection::new(id, 1.))
                .collect::<Vec<Connection<Id>>>(),
        )
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
