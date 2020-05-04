use crate::route::Route;
use std::hash::Hash;
use sample::Sample;

use bufferpool::BufferPoolReference;

pub trait NodeId<Generator>: Hash + Clone + Eq {
    fn generate_node_id(generator: &mut Generator) -> Self;
}

pub struct Connection<Id, S: Sample> {
    pub(crate) id: Id,
    pub(crate) amount: S,
}

impl<Id, S: Sample> Connection<Id, S> {
    pub fn new(id: Id, amount: S) -> Connection<Id, S> {
        Connection { id, amount }
    }

    pub fn id(&self) -> &Id {
        &self.id
    }
}

pub struct Node<Id: NodeId<G>, S: Sample, R: Route<S, C>, C, G> {
    pub(crate) id: Id,
    pub(crate) channels: usize,
    pub(crate) buffers: Vec<BufferPoolReference<S>>,
    pub(crate) connections: Vec<Connection<Id, S>>,
    pub(crate) route: R,
    __context: std::marker::PhantomData<*const C>,
    __id_generator: std::marker::PhantomData<*const G>,
}

impl<Id: NodeId<G>, S: Sample, R: Route<S, C>, C, G> Node<Id, S, R, C, G> {
    pub fn with_id(
        id: Id,
        channels: usize,
        route: R,
        connections: Vec<Connection<Id, S>>,
    ) -> Node<Id, S, R, C, G> {
        Node {
            id,
            channels,
            buffers: Vec::with_capacity(channels),
            route,
            connections,
            __context: Default::default(),
            __id_generator: Default::default(),
        }
    }

    pub fn new(
        channels: usize,
        route: R,
        connections: Vec<Connection<Id, S>>,
        generator: &mut G,
    ) -> Node<Id, S, R, C, G> {
        Self::with_id(
            Id::generate_node_id(generator),
            channels,
            route,
            connections,
        )
    }
}
