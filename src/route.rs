use bufferpool::BufferPoolReference;
use sample::Sample;

pub trait Route<S: Sample, C> {
    fn process(
        &mut self,
        input: &[BufferPoolReference<S>],
        output: &mut [BufferPoolReference<S>],
        frames: usize,
        context: &mut C,
    );
}

impl<S: Sample, C> Route<S, C> for Box<dyn Route<S, C>> {
    fn process(
        &mut self,
        input: &[BufferPoolReference<S>],
        output: &mut [BufferPoolReference<S>],
        frames: usize,
        context: &mut C,
    ) {
        self.as_mut().process(input, output, frames, context);
    }
}
