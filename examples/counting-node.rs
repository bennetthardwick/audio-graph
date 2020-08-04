#[macro_use]
extern crate lazy_static;

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
        type Context = ();

        fn process(
            &mut self,
            _input: &[audiograph::BufferPoolReference<f32>],
            output: &mut [audiograph::BufferPoolReference<f32>],
            _frames: usize,
            _context: &mut Self::Context,
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
        buffer: Vec<f32>,
        offset: usize,
    }

    impl audiograph::Route<f32> for OutputRoute {
        type Context = ();

        fn process(
            &mut self,
            input: &[audiograph::BufferPoolReference<f32>],
            _output: &mut [audiograph::BufferPoolReference<f32>],
            _frames: usize,
            _context: &mut Self::Context,
        ) {
            let buffer = &mut self.buffer;
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
        type Context = ();

        fn process(
            &mut self,
            input: &[audiograph::BufferPoolReference<f32>],
            output: &mut [audiograph::BufferPoolReference<f32>],
            frames: usize,
            context: &mut Self::Context,
        ) {
            match self {
                Routes::Counting(route) => route.process(input, output, frames, context),
                Routes::Output(route) => route.process(input, output, frames, context),
            }
        }
    }

    let test: Vec<f32> = TEST_DATA.iter().cloned().collect();

    let iters = 1000;

    for _ in 0..iters {
        let buffer_size = 1024;
        let mut graph = audiograph::RouteGraphBuilder::new()
            .with_buffer_size(buffer_size)
            .build();

        let output = graph.add_node_with_idx(move |id| {
            audiograph::Node::with_id(
                id,
                1,
                Routes::Output(OutputRoute {
                    buffer: vec![0.; BUFFER_SIZE],
                    offset: 0,
                }),
                vec![],
            )
        });

        graph.add_node_with_idx(|id| {
            audiograph::Node::with_id(
                id,
                1,
                Routes::Counting(CountingRoute { current: 0 }),
                vec![audiograph::Connection::new(output.clone(), 1.)],
            )
        });

        graph.topographic_sort();

        let count = BUFFER_SIZE / buffer_size;

        let mut c = ();

        for _ in 0..count {
            graph.process(buffer_size as usize, &mut c);
        }

        if let Routes::Output(OutputRoute { buffer, .. }) =
            graph.remove_node(output).unwrap().route()
        {
            assert_eq!(*buffer, test);
        } else {
            panic!("Expected output route!");
        }
    }
}
