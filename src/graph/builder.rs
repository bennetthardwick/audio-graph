use super::{Route, RouteGraph};
use generational_arena::Arena;
use sample::Sample;
use std::marker::PhantomData;

pub struct RouteGraphBuilder<S, R, C>
where
    S: Sample + Default,
    R: Route<S, C>,
{
    buffer_size: usize,
    __data: PhantomData<(S, R, C)>,
}

impl<S, R, C> RouteGraphBuilder<S, R, C>
where
    S: Sample + Default,
    R: Route<S, C>,
{
    pub fn new() -> Self {
        Self {
            buffer_size: 1024,
            __data: Default::default(),
        }
    }

    pub fn with_buffer_size(mut self, buffer_size: usize) -> Self {
        self.buffer_size = buffer_size;
        self
    }

    pub fn build(self) -> RouteGraph<S, R, C> {
        RouteGraph::build(Arena::new(), self.buffer_size)
    }
}

impl<S, R, C> Default for RouteGraphBuilder<S, R, C>
where
    S: Sample + Default,
    R: Route<S, C>,
{
    fn default() -> Self {
        RouteGraphBuilder::new()
    }
}
