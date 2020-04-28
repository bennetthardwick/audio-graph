use crate::route::Route;
use sample::Sample;

use bufferpool::BufferPoolReference;

pub trait NodeId {
    fn generate_node_id() -> Self;
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

pub struct Node<Id: NodeId, S: Sample, R: Route<S, C>, C> {
    pub(crate) id: Id,
    pub(crate) channels: usize,
    pub(crate) buffers: Vec<BufferPoolReference<S>>,
    pub(crate) connections: Vec<Connection<Id, S>>,
    pub(crate) route: R,
    __context: std::marker::PhantomData<*const C>,
}

impl<Id: NodeId, S: Sample, R: Route<S, C>, C> Node<Id, S, R, C> {
    pub fn with_id(
        id: Id,
        channels: usize,
        route: R,
        connections: Vec<Connection<Id, S>>,
    ) -> Node<Id, S, R, C> {
        Node {
            id,
            channels,
            buffers: Vec::with_capacity(channels),
            route,
            connections,
            __context: std::marker::PhantomData::default(),
        }
    }

    pub fn new(
        channels: usize,
        route: R,
        connections: Vec<Connection<Id, S>>,
    ) -> Node<Id, S, R, C> {
        Self::with_id(Id::generate_node_id(), channels, route, connections)
    }
}
