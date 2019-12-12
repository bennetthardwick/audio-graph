use bufferpool::BufferPoolReference;
use core::fmt::Debug;
use sample::Sample;
use std::any::Any;

pub trait Route<S: Sample>: Debug {
    fn process(
        &mut self,
        input: &[BufferPoolReference<S>],
        output: &mut [BufferPoolReference<S>],
        frames: usize,
    );

    fn as_any(&self) -> &dyn Any;
}
