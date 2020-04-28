#[macro_use]
extern crate lazy_static;

use std::cell::RefCell;
use std::rc::Rc;

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

    impl audiograph::Route<f32> for CountingRoute {
        fn process(
            &mut self,
            _input: &[audiograph::BufferPoolReference<f32>],
            output: &mut [audiograph::BufferPoolReference<f32>],
            _frames: usize,
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

    impl audiograph::Route<f32> for OutputRoute {
        fn process(
            &mut self,
            input: &[audiograph::BufferPoolReference<f32>],
            _output: &mut [audiograph::BufferPoolReference<f32>],
            _frames: usize,
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

    impl audiograph::Route<f32> for Routes {
        fn process(
            &mut self,
            input: &[audiograph::BufferPoolReference<f32>],
            output: &mut [audiograph::BufferPoolReference<f32>],
            frames: usize,
        ) {
            match self {
                Routes::Counting(route) => route.process(input, output, frames),
                Routes::Output(route) => route.process(input, output, frames),
            }
        }
    }

    #[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
    struct Id(u32);

    impl audiograph::NodeId for Id {
        fn generate_node_id() -> Self {
            Id(0)
        }
    }

    let test: Vec<f32> = TEST_DATA.iter().cloned().collect();

    let iters = 10000;

    for _ in 0..iters {
        let buffer: Vec<f32> = vec![0.; BUFFER_SIZE];
        let buffer = Rc::new(RefCell::new(buffer));

        let output_id = Id(1);

        let buffer_size = 1024;
        let count = BUFFER_SIZE / buffer_size;

        let mut graph = audiograph::RouteGraph::with_nodes(
            vec![
                audiograph::Node::new(
                    1,
                    Routes::Counting(CountingRoute { current: 0 }),
                    vec![audiograph::Connection::new(output_id, 1.)],
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

        for _ in 0..count {
            graph.process(buffer_size as usize);
        }

        assert_eq!(*buffer.borrow(), test);
    }
}
