use crate::route::Route;
use generational_arena::Index;
use sample::Sample;

use bufferpool::BufferPoolReference;

pub struct Connection<S> {
    pub(crate) id: Index,
    pub(crate) amount: S,
}

impl<S: Sample> Connection<S> {
    pub fn new(id: Index, amount: S) -> Connection<S> {
        Connection { id, amount }
    }

    pub fn id(&self) -> Index {
        self.id
    }
}

pub struct Node<S, R> {
    pub(crate) id: Index,
    pub(crate) channels: usize,
    pub(crate) buffers: Vec<BufferPoolReference<S>>,
    pub(crate) connections: Vec<Connection<S>>,
    pub(crate) route: R,
}

impl<S, R, C> Node<S, R>
where
    S: Sample,
    R: Route<S, Context = C>,
{
    pub fn id(&self) -> Index {
        self.id
    }

    pub fn route(&mut self) -> &mut R {
        &mut self.route
    }

    pub fn with_id(
        id: Index,
        channels: usize,
        route: R,
        connections: Vec<Connection<S>>,
    ) -> Node<S, R> {
        Node {
            id,
            channels,
            buffers: Vec::with_capacity(channels),
            route,
            connections,
        }
    }
}
