#[macro_use]
extern crate lazy_static;

use audiograph::NodeId;
use std::cell::RefCell;
use std::rc::Rc;
use volatile_unique_id::*;

use audiograph;

const BUFFER_SIZE: usize = 1024 * 1024;

lazy_static! {
    static ref TEST_DATA: Vec<f32> = {
        let mut test: Vec<f32> = vec![0.; BUFFER_SIZE];
        for (index, value) in test.iter_mut().enumerate() {
            *value = index as f32;
        }
        test
    };
}

fn main() {
    #[derive(Debug)]
    struct CountingRoute {
        current: usize,
    }

    type C = ();

    impl audiograph::Route<f32, C> for CountingRoute {
        fn process(
            &mut self,
            _input: &[audiograph::BufferPoolReference<f32>],
            output: &mut [audiograph::BufferPoolReference<f32>],
            _frames: usize,
            _context: &mut C,
        ) {
            let current = self.current;
            for (index, sample) in output[0].as_mut().iter_mut().enumerate() {
                *sample = (current + index) as f32;
            }

            self.current += output[0].as_ref().len();
        }
    }

    #[derive(Debug)]
    struct OutputRoute {
        buffer: Rc<RefCell<Vec<f32>>>,
        offset: usize,
    }

    impl audiograph::Route<f32, C> for OutputRoute {
        fn process(
            &mut self,
            input: &[audiograph::BufferPoolReference<f32>],
            _output: &mut [audiograph::BufferPoolReference<f32>],
            _frames: usize,
            _context: &mut C,
        ) {
            let mut buffer = self.buffer.borrow_mut();
            for (input, output) in input[0]
                .as_ref()
                .iter()
                .zip(buffer[self.offset..].iter_mut())
            {
                *output = *input;
            }

            self.offset += input[0].as_ref().len();
        }
    }

    enum Routes {
        Counting(CountingRoute),
        Output(OutputRoute),
    }

    impl audiograph::Route<f32, C> for Routes {
        fn process(
            &mut self,
            input: &[audiograph::BufferPoolReference<f32>],
            output: &mut [audiograph::BufferPoolReference<f32>],
            frames: usize,
            context: &mut C,
        ) {
            match self {
                Routes::Counting(route) => route.process(input, output, frames, context),
                Routes::Output(route) => route.process(input, output, frames, context),
            }
        }
    }

    #[derive(Debug, Eq, PartialEq, Clone, Hash)]
    struct Id(volatile_unique_id::Id);

    impl audiograph::NodeId<Generator> for Id {
        fn generate_node_id(generator: &mut Generator) -> Self {
            Id(generator.generate())
        }
    }

    let test: Vec<f32> = TEST_DATA.iter().cloned().collect();

    let iters = 10000;

    let mut generator = GeneratorBuilder::new().build();

    for _ in 0..iters {
        let buffer: Vec<f32> = vec![0.; BUFFER_SIZE];
        let buffer = Rc::new(RefCell::new(buffer));

        let output_id = Id::generate_node_id(&mut generator);

        let buffer_size = 1024;
        let count = BUFFER_SIZE / buffer_size;

        let mut graph = audiograph::RouteGraph::with_nodes(
            vec![
                audiograph::Node::new(
                    1,
                    Routes::Counting(CountingRoute { current: 0 }),
                    vec![audiograph::Connection::new(output_id.clone(), 1.)],
                    &mut generator,
                ),
                audiograph::Node::with_id(
                    output_id,
                    1,
                    Routes::Output(OutputRoute {
                        buffer: Rc::clone(&buffer),
                        offset: 0,
                    }),
                    vec![],
                ),
            ],
            buffer_size as usize,
        );

        let mut c = ();

        for _ in 0..count {
            graph.process(buffer_size as usize, &mut c);
        }

        assert_eq!(*buffer.borrow(), test);
    }
}
