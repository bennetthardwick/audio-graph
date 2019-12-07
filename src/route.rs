use sample::Sample;

pub trait Route<S: Sample> {
    fn process(&mut self, input: &[&[S]], output: &[&mut [S]], frames: usize);
}

