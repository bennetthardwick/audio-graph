use crate::route::Route;
use sample::Sample;

use bufferpool::BufferPoolReference;

pub trait NodeId {
    fn generate_node_id() -> Self;
}

pub struct Connection<Id> {
    pub(crate) id: Id,
    amount: f32,
}

impl<Id> Connection<Id> {
    pub fn new(id: Id, amount: f32) -> Connection<Id> {
        Connection { id, amount }
    }

    pub fn id(&self) -> &Id {
        &self.id
    }
}

pub struct Node<Id: NodeId, S: Sample> {
    pub(crate) id: Id,
    channels: usize,
    buffers: Vec<BufferPoolReference<S>>,
    pub(crate) connections: Vec<Connection<Id>>,
    route: Box<dyn Route<S>>,
}

impl<Id: NodeId, S: Sample> Node<Id, S> {
    pub fn with_id(
        id: Id,
        channels: usize,
        route: Box<dyn Route<S>>,
        connections: Vec<Connection<Id>>,
    ) -> Node<Id, S> {
        Node {
            id,
            channels,
            buffers: Vec::with_capacity(channels),
            route,
            connections,
        }
    }

    pub fn new(
        channels: usize,
        route: Box<dyn Route<S>>,
        connections: Vec<Connection<Id>>,
    ) -> Node<Id, S> {
        Self::with_id(Id::generate_node_id(), channels, route, connections)
    }
}
