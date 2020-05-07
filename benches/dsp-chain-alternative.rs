#![feature(test)]
extern crate test;

#[macro_use]
extern crate lazy_static;

use dsp::Node;
use std::cell::RefCell;
use std::rc::Rc;
use test::Bencher;

use audiograph;
use dsp;

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

#[bench]
fn bench_dsp_chain_count_to_max(b: &mut Bencher) {
    struct CountingNode;

    impl dsp::Node<[f32; 1]> for CountingNode {
        fn audio_requested(&mut self, buffer: &mut [[f32; 1]], _sample_hz: f64) {
            for (index, sample) in buffer.iter_mut().enumerate() {
                sample[0] = index as f32;
            }
        }
    }

    let test: Vec<[f32; 1]> = TEST_DATA.iter().cloned().map(|x| [x; 1]).collect();

    b.iter(|| {
        let mut buffer: Vec<[f32; 1]> = vec![[0.; 1]; BUFFER_SIZE];
        let mut graph = dsp::Graph::new();
        let counter = graph.add_node(CountingNode);
        graph.set_master(Some(counter));
        graph.audio_requested(&mut buffer, 44100.0);
        assert_eq!(buffer, test);
    });
}

#[bench]
fn bench_audiograph_count_to_max(b: &mut Bencher) {
    #[derive(Debug)]
    struct CountingRoute {
        current: usize,
    }

    impl audiograph::Route<f32, ()> for CountingRoute {
        fn process(
            &mut self,
            _input: &[audiograph::BufferPoolReference<f32>],
            output: &mut [audiograph::BufferPoolReference<f32>],
            _frames: usize,
            _context: &mut (),
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

    impl audiograph::Route<f32, ()> for OutputRoute {
        fn process(
            &mut self,
            input: &[audiograph::BufferPoolReference<f32>],
            _output: &mut [audiograph::BufferPoolReference<f32>],
            _frames: usize,
            _context: &mut (),
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

    impl audiograph::Route<f32, ()> for Routes {
        fn process(
            &mut self,
            input: &[audiograph::BufferPoolReference<f32>],
            output: &mut [audiograph::BufferPoolReference<f32>],
            frames: usize,
            context: &mut (),
        ) {
            match self {
                Routes::Counting(route) => route.process(input, output, frames, context),
                Routes::Output(route) => route.process(input, output, frames, context),
            }
        }
    }

    let test: Vec<f32> = TEST_DATA.iter().cloned().collect();

    b.iter(|| {
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

        if let Routes::Output(OutputRoute { buffer, .. }) = graph.remove_node(output).route() {
            assert_eq!(*buffer, test);
        } else {
            panic!("Expected output route!");
        }
    });
}
