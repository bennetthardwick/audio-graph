use bufferpool::BufferPoolReference;
use sample::Sample;

pub trait Route<S: Sample> {
    type Context;

    fn process(
        &mut self,
        input: &[BufferPoolReference<S>],
        output: &mut [BufferPoolReference<S>],
        frames: usize,
        context: &mut Self::Context,
    );
}

impl<S: Sample, C> Route<S> for Box<dyn Route<S, Context = C>> {
    type Context = C;

    fn process(
        &mut self,
        input: &[BufferPoolReference<S>],
        output: &mut [BufferPoolReference<S>],
        frames: usize,
        context: &mut Self::Context,
    ) {
        self.as_mut().process(input, output, frames, context);
    }
}
