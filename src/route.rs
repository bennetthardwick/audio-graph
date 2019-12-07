pub struct RouteSend<RouteId> {
    pub id: RouteId,
    pub amount: f32,
}

trait Zero {
    fn zero(&mut self);
}

impl Zero for u32 {
    fn zero(&mut self) {
        *self = 0;
    }
}

impl Zero for u8 {
    fn zero(&mut self) {
        *self = 0;
    }
}

impl Zero for f32 {
    fn zero(&mut self) {
        *self = 0.;
    }
}

pub trait RouteProxy<RouteId: Copy, ChannelId: Copy, S: Zero> {
    fn id(&self) -> RouteId;
    fn activate(
        self: Box<Self>,
    ) -> (
        Box<dyn ActivatedRouteProxy<RouteId, ChannelId, S>>,
        Box<dyn Route<RouteId, ChannelId, S>>,
    );
}

pub trait ActivatedRouteProxy<RouteId: Copy, ChannelId: Copy, S: Zero> {
    fn id(&self) -> RouteId;
}

pub trait Route<RouteId: Copy, ChannelId: Copy, S: Zero> {
    fn id(&self) -> RouteId;
    fn buffer(&self, channel: &ChannelId) -> Option<&[S]>;
    fn buffer_mut(&mut self, channel: &ChannelId) -> Option<&mut [S]>;
    fn buffer_size(&self) -> usize;
    fn full_buffer_mut(&mut self) -> &mut [S];
    fn in_channels(&self) -> &[ChannelId];
    fn out_channels(&self) -> &[ChannelId];
    fn out_routes(&self) -> &[RouteSend<RouteId>];
    fn activate_channel(&mut self, channel: ChannelId);
    fn deactivate_channel(&mut self, channel: ChannelId);
    fn process(&mut self);
    fn resize_buffers(&mut self, size: usize);
    fn silence_all_buffers(&mut self) {
        self.full_buffer_mut().iter_mut().for_each(|x| x.zero());
    }
}
