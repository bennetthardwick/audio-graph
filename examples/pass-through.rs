use audiograph::*;
use std::io::Read;
use volatile_unique_id::*;

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

// In order to pass information from the audio backend (in this example Jack)
// a "context" object is used. This object contains everything that the routes
// in the graph might need, with each route getting a mutable reference to it.
//
// The jack context passes through the process scope as well as references to
// the input and output ports.
//
// Note: due to some lifetime issues with Rust, a bit of unsafe code is needed,
// see here: https://users.rust-lang.org/t/phantomdata-const-t-causing-issues-with-lifetimes/41708
struct Context {
    in_l_port: *const jack::Port<jack::AudioIn>,
    in_r_port: *const jack::Port<jack::AudioIn>,

    out_l_port: *mut jack::Port<jack::AudioOut>,
    out_r_port: *mut jack::Port<jack::AudioOut>,

    ps: *const jack::ProcessScope,
}

impl Context {
    // Get the two buffers of f32 that come from Jack
    fn get_audio_input(&self) -> [&[f32]; 2] {
        unsafe {
            let ps = &*self.ps;

            let left = (&*self.in_l_port).as_slice(ps);
            let right = (&*self.in_r_port).as_slice(ps);

            [left, right]
        }
    }

    // Get the two buffers of f32 that Jack will play once we fill
    // them with audio.
    fn get_audio_output(&mut self) -> [&mut [f32]; 2] {
        unsafe {
            let ps = &*self.ps;

            let left = (&mut *self.out_l_port).as_mut_slice(ps);
            let right = (&mut *self.out_r_port).as_mut_slice(ps);

            [left, right]
        }
    }
}

// Create a route for input. This recieves the audio from Jack and
// passes it to our graph.
struct InputRoute;

// Implement route for the InputRoute
impl Route<Sample, Context> for InputRoute {
    fn process(
        &mut self,
        _input: &[BufferPoolReference<Sample>],
        output: &mut [BufferPoolReference<Sample>],
        _frames: usize,
        context: &mut Context,
    ) {
        for (output_stream, input_stream) in output.iter_mut().zip(context.get_audio_input().iter())
        {
            for (out_sample, in_sample) in
                output_stream.as_mut().iter_mut().zip(input_stream.iter())
            {
                *out_sample = *in_sample;
            }
        }
    }
}

// Much like the InputRoute, create a route so that data can be
// sent from within the graph back to the outside world.
struct OutputRoute;

impl Route<Sample, Context> for OutputRoute {
    fn process(
        &mut self,
        input: &[BufferPoolReference<Sample>],
        _output: &mut [BufferPoolReference<Sample>],
        _frames: usize,
        context: &mut Context,
    ) {
        for (output_stream, input_stream) in
            context.get_audio_output().iter_mut().zip(input.as_ref())
        {
            for (out_sample, in_sample) in output_stream.iter_mut().zip(input_stream.as_ref()) {
                *out_sample = *in_sample;
            }
        }
    }
}

// RouteGraph also requires that each node have a unique id.
// What Id you use is completely up to you, in this example I use
// a library called uuid.
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
struct Id(volatile_unique_id::Id);

impl NodeId<Generator> for Id {
    fn generate_node_id(generator: &mut Generator) -> Self {
        Id(generator.generate())
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
impl<'a> Route<Sample, Context> for Routes {
    fn process(
        &mut self,
        input: &[BufferPoolReference<Sample>],
        output: &mut [BufferPoolReference<Sample>],
        frames: usize,
        context: &mut Context,
    ) {
        match self {
            Routes::Input(r) => r.process(input, output, frames, context),
            Routes::Output(r) => r.process(input, output, frames, context),
        }
    }
}

fn main() {
    let client = jack::Client::new(APP_NAME, jack::ClientOptions::NO_START_SERVER)
        .unwrap()
        .0;

    let channels = 2;

    let mut generator = GeneratorBuilder::new().build();

    // Create a channel to send data from Jack into the route.
    let output_id = Id::generate_node_id(&mut generator);

    // Create the Node to host the route. Nodes have a little bit of extra information
    // that is used with the routing of the graph, such as the number of channels it has
    // and the other nodes that it's connected to.
    let output_node: Node<Id, Sample, Routes, _, _> =
        Node::with_id(output_id.clone(), channels, Routes::Output(OutputRoute), vec![]);

    let input_id = Id::generate_node_id(&mut generator);
    let input_node = Node::with_id(
        input_id,
        channels,
        Routes::Input(InputRoute),
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
    let process = jack::ClosureProcessHandler::new(
        move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
            let frames = ps.n_frames() as usize;

            let mut context = Context {
                out_r_port: &mut out_r_port,
                out_l_port: &mut out_l_port,
                in_l_port: &in_l_port,
                in_r_port: &in_r_port,
                ps,
            };

            graph.process(frames, &mut context);

            jack::Control::Continue
        },
    );

    // Activate the graph and start processing audio
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

    // Connect the output to the system output.. sorry, you'll need to
    // wire up the inputs yourself (with something like Catia)
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
