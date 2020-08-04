use super::{Route, RouteGraph};
use generational_arena::Arena;
use sample::Sample;
use std::marker::PhantomData;

pub struct RouteGraphBuilder<S, R>
where
    S: Sample + Default,
{
    buffer_size: usize,
    __data: PhantomData<(S, R)>,
}

impl<S, R> RouteGraphBuilder<S, R>
where
    S: Sample + Default,
    R: Route<S>,
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

    pub fn build(self) -> RouteGraph<S, R> {
        RouteGraph::build(Arena::new(), self.buffer_size)
    }
}

impl<S, R> Default for RouteGraphBuilder<S, R>
where
    S: Sample + Default,
    R: Route<S>,
{
    fn default() -> Self {
        RouteGraphBuilder::new()
    }
}
