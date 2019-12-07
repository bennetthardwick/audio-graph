use sample::Sample;
use crate::route::Route;

struct Node<'a, S: Sample, R: Route<S>> {
    channels: usize,
    buffers: Option<&'a [&'a [S]]>,
    route: R,
}
