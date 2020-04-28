use audiograph::*;
use crossbeam::{bounded, channel::Receiver, channel::Sender};
use std::io::Read;
use uuid::Uuid;

// Define some strings to be used throughout the application
const APP_NAME: &str = "pass-through-audiograph";
const OUT_L: &str = "out-l";
const OUT_R: &str = "out-r";
const IN_L: &str = "in-l";
const IN_R: &str = "in-r";

// Define the sample type to be used throughout the application.
// Since Jack uses f32s, we'll do the same.
type Sample = f32;

// Routes are the basis of everything in RouteGraph.
// To create input and to hear output a route needs to be created.
// This is down by implementing the Route<Sample> trait.

// Create a route for input. This will receive the audio from
// Jack and pass it to our graph.
struct InputRoute {
    // The crossbeam channel is a great way to send data
    // around - especially between threads.
    input: Receiver<Vec<&'static [Sample]>>,
    returner: Sender<Vec<&'static [Sample]>>,
}

struct Context<'a> {
    pub(crate) in_l_port: &'a jack::Port<jack::AudioIn>,
    pub(crate) in_r_port: &'a jack::Port<jack::AudioIn>,

    pub(crate) out_l_port: &'a mut jack::Port<jack::AudioOut>,
    pub(crate) out_r_port: &'a mut jack::Port<jack::AudioOut>,

    pub(crate) ps: &'a jack::ProcessScope,
}

// Implement route for the InputRoute
impl Route<Sample, Context<'_>> for InputRoute {
    fn process(
        &mut self,
        _input: &[BufferPoolReference<Sample>],
        output: &mut [BufferPoolReference<Sample>],
        frames: usize,
        context: &mut Context,
    ) {
        if let Some(data) = self.input.try_iter().last() {
            for (output_stream, input_stream) in output.iter_mut().zip(data.iter()) {
                // Copy all the data across.
                unsafe {
                    let len = output_stream
                        .as_ref()
                        .len()
                        .min(input_stream.len())
                        .min(frames);
                    let dst = output_stream.as_mut().as_mut_ptr();
                    let src = input_stream.as_ptr();
                    std::ptr::copy_nonoverlapping(src, dst, len);
                }
            }

            self.returner.try_send(data).unwrap();
        }
    }
}

// Much like the InputRoute, create a route so that data can be
// sent from within the graph back to the outside world.
struct OutputRoute {
    output: Receiver<Vec<&'static mut [Sample]>>,
    returner: Sender<Vec<&'static mut [Sample]>>,
}

impl Route<Sample, Context<'_>> for OutputRoute {
    fn process(
        &mut self,
        input: &[BufferPoolReference<Sample>],
        _output: &mut [BufferPoolReference<Sample>],
        frames: usize,
        context: &mut Context,
    ) {
        if let Some(mut data) = self.output.try_iter().last() {
            for (output_stream, input_stream) in data.iter_mut().zip(input.iter()) {
                // Copy all the data across.
                unsafe {
                    let len = output_stream
                        .len()
                        .min(input_stream.as_ref().len())
                        .min(frames);
                    let dst = output_stream.as_mut_ptr();
                    let src = input_stream.as_ref().as_ptr();
                    std::ptr::copy_nonoverlapping(src, dst, len);
                }
            }

            self.returner.try_send(data).unwrap();
        }
    }
}

// RouteGraph also requires that each node have a unique id.git@github.com:bennetthardwick/audio-graph.git
// What Id you use is completely up to you, in this example I usea
// a library called uuid.
#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
struct Id(Uuid);

impl NodeId for Id {
    fn generate_node_id() -> Self {
        Id(Uuid::new_v4())
    }
}

// There's many ways to represent routes inside the graph. To minimise
// the amount of heap allocations, I've opted to use an enum instead of
// Box<dyn Route<Sample>>. This also helps if I want to convert the route
// back into it's original value.
enum Routes {
    Input(InputRoute),
    Output(OutputRoute),
}

// Likewise, implement Route<Sample> for the Routes enum.
impl<'a> Route<Sample, Context<'a>> for Routes {
    fn process(
        &mut self,
        input: &[BufferPoolReference<Sample>],
        output: &mut [BufferPoolReference<Sample>],
        frames: usize,
        context: &mut Context,
    ) {
        match self {
            Routes::Input(route) => route.process(input, output, frames, context),
            Routes::Output(route) => route.process(input, output, frames, context),
        }
    }
}

fn main() {
    let client = jack::Client::new(APP_NAME, jack::ClientOptions::NO_START_SERVER)
        .unwrap()
        .0;

    let channels = 2;

    // A channel to send buffers back after being used

    let (return_input_send, return_input_recv) = bounded::<Vec<&'static [Sample]>>(1);
    let (return_output_send, return_output_recv) = bounded::<Vec<&'static mut [Sample]>>(1);

    // Fill up the channel with some buffers to be used in the loop
    return_output_send
        .try_send(Vec::with_capacity(channels))
        .unwrap();
    return_input_send
        .try_send(Vec::with_capacity(channels))
        .unwrap();

    // Create a channel to send data from Jack into the route.
    let (output_send, output_recv) = bounded::<Vec<&'static mut [Sample]>>(1);
    let output_id = Id::generate_node_id();

    // Create the Node to host the route. Nodes have a little bit of extra information
    // that is used with the routing of the graph, such as the number of channels it has
    // and the other nodes that it's connected to.
    let output_node: Node<Id, Sample, Routes, _> = Node::with_id(
        output_id,
        channels,
        Routes::Output(OutputRoute {
            output: output_recv,
            returner: return_output_send,
        }),
        vec![],
    );

    let (input_send, input_recv) = bounded::<Vec<&'static [Sample]>>(1);
    let input_id = Id::generate_node_id();
    let input_node = Node::with_id(
        input_id,
        channels,
        Routes::Input(InputRoute {
            input: input_recv,
            returner: return_input_send,
        }),
        // Connect to the output route with an amplitude of 1
        vec![Connection::new(output_id, 1.)],
    );

    // The initial buffer size for the graph. With Jack this can change all the time
    // after the graph has been created - but this example doesn't support that.
    let buffer_size = client.buffer_size();

    // Create a graph of just the input and output nodes.
    let mut graph = RouteGraph::with_nodes(vec![input_node, output_node], buffer_size as usize);

    // Get the specifications for input and output Jack ports
    let out_spec = jack::AudioOut::default();
    let in_spec = jack::AudioIn::default();

    // Register the left and right out channels
    let mut out_l_port = client.register_port(OUT_L, out_spec).unwrap();
    let mut out_r_port = client.register_port(OUT_R, out_spec).unwrap();

    // Register the left and right in channels
    let in_l_port = client.register_port(IN_L, in_spec).unwrap();
    let in_r_port = client.register_port(IN_R, in_spec).unwrap();

    // Create the Jack callback. This function is called for every buffer that is requested from
    // Jack. It's responsibility is to send the slices to the input and output routes and then
    // process the graph.
    //

    let process = jack::ClosureProcessHandler::new(
        move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
            let frames = ps.n_frames();

            let mut context = Context {
                out_r_port: &mut out_r_port,
                out_l_port: &mut out_l_port,
                in_l_port: &in_l_port,
                in_r_port: &in_r_port,
                ps,
            };

            graph.process(frames as usize, &mut context);

            // drop(context);

            // unsafe {
            //     if let Some(mut in_vec) = return_input_recv.try_iter().last() {
            //         in_vec.clear();
            //         in_vec.push(std::slice::from_raw_parts(in_l.as_ptr(), in_l.len()));
            //         in_vec.push(std::slice::from_raw_parts(in_r.as_ptr(), in_r.len()));
            //         input_send.try_send(in_vec).unwrap();
            //     }

            //     if let Some(mut out_vec) = return_output_recv.try_iter().last() {
            //         out_vec.clear();
            //         out_vec.push(std::slice::from_raw_parts_mut(
            //             out_l.as_mut_ptr(),
            //             out_l.len(),
            //         ));
            //         out_vec.push(std::slice::from_raw_parts_mut(
            //             out_r.as_mut_ptr(),
            //             out_r.len(),
            //         ));
            //         output_send.try_send(out_vec).unwrap();
            //     }
            // }

            // // Process the graph
            // graph.process(in_l.len().min(out_l.len()), &mut context);

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
