use bufferpool::BufferPoolReference;
use sample::Sample;

pub trait Route<S: Sample> {
    fn process(
        &mut self,
        input: &[BufferPoolReference<S>],
        output: &mut [BufferPoolReference<S>],
        frames: usize,
    );
}

impl<S: Sample> Route<S> for Box<dyn Route<S>> {
    fn process(
        &mut self,
        input: &[BufferPoolReference<S>],
        output: &mut [BufferPoolReference<S>],
        frames: usize,
    ) {
        (**self).process(input, output, frames);
    }
}
