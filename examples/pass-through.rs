use audiograph::*;
use crossbeam::{bounded, channel::Receiver};
use std::io::Read;
use uuid::Uuid;

const APP_NAME: &str = "pass-through-audiograph";
const OUT_L: &str = "out-l";
const OUT_R: &str = "out-r";

const IN_L: &str = "in-l";
const IN_R: &str = "in-r";

type S = f32;

struct InputRoute {
    input: Receiver<Vec<&'static [S]>>,
}

impl Route<S> for InputRoute {
    fn process(
        &mut self,
        _input: &[BufferPoolReference<S>],
        output: &mut [BufferPoolReference<S>],
        frames: usize,
    ) {
        if let Some(data) = self.input.try_iter().last() {
            for (output_stream, input_stream) in output.iter_mut().zip(data.iter()) {
                for (output_sample, input_sample) in output_stream
                    .as_mut()
                    .iter_mut()
                    .zip(input_stream.iter())
                    .take(frames)
                {
                    *output_sample = *input_sample;
                }
            }
        }
    }
}

struct OutputRoute {
    output: Receiver<Vec<&'static mut [S]>>,
}

impl Route<S> for OutputRoute {
    fn process(
        &mut self,
        input: &[BufferPoolReference<S>],
        _output: &mut [BufferPoolReference<S>],
        frames: usize,
    ) {
        if let Some(mut data) = self.output.try_iter().last() {
            for (output_stream, input_stream) in data.iter_mut().zip(input.iter()) {
                for (output_sample, input_sample) in output_stream
                    .iter_mut()
                    .zip(input_stream.as_ref().iter())
                    .take(frames)
                {
                    *output_sample = *input_sample;
                }
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
struct Id(Uuid);

impl NodeId for Id {
    fn generate_node_id() -> Self {
        Id(Uuid::new_v4())
    }
}

enum Routes {
    Input(InputRoute),
    Output(OutputRoute),
}

impl Route<S> for Routes {
    fn process(
        &mut self,
        input: &[BufferPoolReference<S>],
        output: &mut [BufferPoolReference<S>],
        frames: usize,
    ) {
        match self {
            Routes::Input(route) => route.process(input, output, frames),
            Routes::Output(route) => route.process(input, output, frames),
        }
    }
}

fn main() {
    let client = jack::Client::new(
        "pass-through-audiograph",
        jack::ClientOptions::NO_START_SERVER,
    )
    .unwrap()
    .0;

    let channels = 2;

    let (output_send, output_recv) = bounded::<Vec<&'static mut [S]>>(1);
    let output_id = Id::generate_node_id();
    let output_node: Node<Id, S, Routes> = Node::with_id(
        output_id,
        channels,
        Routes::Output(OutputRoute {
            output: output_recv,
        }),
        vec![],
    );

    let (input_send, input_recv) = bounded::<Vec<&'static [S]>>(1);
    let input_id = Id::generate_node_id();
    let input_node = Node::with_id(
        input_id,
        channels,
        Routes::Input(InputRoute { input: input_recv }),
        vec![Connection::new(output_id, 1.)],
    );

    let buffer_size = client.buffer_size();

    let mut graph: RouteGraph<Id, S, Routes> =
        RouteGraph::with_nodes(vec![input_node, output_node], buffer_size as usize);

    // Node::with_id(input_id, Box::new(

    let out_spec = jack::AudioOut::default();
    let in_spec = jack::AudioIn::default();

    let mut out_l_port = client.register_port(OUT_L, out_spec).unwrap();
    let mut out_r_port = client.register_port(OUT_R, out_spec).unwrap();

    let in_l_port = client.register_port(IN_L, in_spec).unwrap();
    let in_r_port = client.register_port(IN_R, in_spec).unwrap();

    let process = jack::ClosureProcessHandler::new(
        move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
            let in_l = in_l_port.as_slice(ps);
            let in_r = in_r_port.as_slice(ps);

            let out_l = out_l_port.as_mut_slice(ps);
            let out_r = out_r_port.as_mut_slice(ps);

            unsafe {
                input_send
                    .try_send(vec![
                        std::slice::from_raw_parts(in_l.as_ptr(), in_l.len()),
                        std::slice::from_raw_parts(in_r.as_ptr(), in_r.len()),
                    ])
                    .unwrap();

                output_send
                    .try_send(vec![
                        std::slice::from_raw_parts_mut(out_l.as_mut_ptr(), out_l.len()),
                        std::slice::from_raw_parts_mut(out_r.as_mut_ptr(), out_r.len()),
                    ])
                    .unwrap();
            }

            graph.process(in_l.len().min(out_l.len()));

            jack::Control::Continue
        },
    );

    let active = client.activate_async((), process).unwrap();

    let system_out_1 = active
        .as_client()
        .port_by_name("system:playback_1")
        .unwrap();
    let system_out_2 = active
        .as_client()
        .port_by_name("system:playback_2")
        .unwrap();
    let out_l_port = active
        .as_client()
        .port_by_name(format!("{}:{}", APP_NAME, OUT_L).as_str())
        .unwrap();
    let out_r_port = active
        .as_client()
        .port_by_name(format!("{}:{}", APP_NAME, OUT_R).as_str())
        .unwrap();
    active
        .as_client()
        .connect_ports(&out_l_port, &system_out_1)
        .unwrap();
    active
        .as_client()
        .connect_ports(&out_r_port, &system_out_2)
        .unwrap();

    // Wait for a character!
    let mut stdin = std::io::stdin();
    let _ = stdin.read(&mut [0u8]).unwrap();

    active.deactivate().unwrap();
}
