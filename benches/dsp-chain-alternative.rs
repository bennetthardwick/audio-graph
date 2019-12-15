#![feature(test)]
extern crate test;

#[macro_use]
extern crate lazy_static;

use dsp::Node;
use sample::*;
use std::any::Any;
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
    struct CountingNode {
        current: usize,
    }

    impl audiograph::Route<f32> for CountingNode {
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

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[derive(Debug)]
    struct OutputNode {
        buffer: Rc<RefCell<Vec<f32>>>,
        offset: usize,
    }

    impl audiograph::Route<f32> for OutputNode {
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

        fn as_any(&self) -> &dyn Any {
            self
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

    b.iter(|| {
        let buffer: Vec<f32> = vec![0.; BUFFER_SIZE];
        let buffer = Rc::new(RefCell::new(buffer));

        let output_id = Id(1);

        let buffer_size = 1024;
        let count = BUFFER_SIZE / buffer_size;

        let mut graph = audiograph::RouteGraph::with_nodes(
            vec![
                audiograph::Node::new(
                    1,
                    Box::new(CountingNode { current: 0 }),
                    vec![audiograph::Connection::new(output_id, 1.)],
                ),
                audiograph::Node::with_id(
                    output_id,
                    1,
                    Box::new(OutputNode {
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
    });
}
