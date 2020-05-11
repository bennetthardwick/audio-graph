use crate::route::Route;
use generational_arena::Index;
use sample::Sample;

use bufferpool::BufferPoolReference;

pub struct Connection<S: Sample> {
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

pub struct Node<S: Sample, R: Route<S, C>, C> {
    pub(crate) id: Index,
    pub(crate) channels: usize,
    pub(crate) buffers: Vec<BufferPoolReference<S>>,
    pub(crate) connections: Vec<Connection<S>>,
    pub(crate) route: R,
    __context: std::marker::PhantomData<*const C>,
}

impl<S: Sample, R: Route<S, C>, C> Node<S, R, C> {

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
    ) -> Node<S, R, C> {
        Node {
            id,
            channels,
            buffers: Vec::with_capacity(channels),
            route,
            connections,
            __context: Default::default(),
        }
    }
}
