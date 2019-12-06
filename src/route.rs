use uuid::Uuid;

pub type RouteId = Uuid;
pub type ChannelId = Uuid;

pub struct RouteSend {
    pub id: RouteId,
    pub amount: f32,
}

pub trait Route {
    fn id(&self) -> &RouteId;
    fn buffer(&self, channel: ChannelId) -> Option<&[f32]>;
    fn buffer_mut(&mut self, channel: ChannelId) -> Option<&mut [f32]>;
    fn buffer_size(&self) -> usize;
    fn in_channels(&self) -> usize;
    fn out_channels(&self) -> usize;
    fn out_routes(&self) -> &[RouteSend];
    fn activate_channel(&mut self, channel: ChannelId);
    fn deactivate_channel(&mut self, channel: ChannelId);
    fn process(&mut self);
    fn resize_buffers(&mut self, size: usize);
}
