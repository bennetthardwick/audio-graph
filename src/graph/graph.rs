use crate::route::Route;
use log::{error, warn};
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::hash::Hash;

// pub struct RouteGraph<RouteId, ChannelId> {
//     ordering: Vec<RouteId>,
//     temp_ordering: Vec<RouteId>,
//     stack: Vec<RouteId>,
//     visited: HashSet<RouteId>,
// 
//     routes: Vec<RouteId>,
//     route_map: HashMap<RouteId, Box<dyn Route<RouteId, ChannelId>>>,
// 
//     sorted: bool,
// }
// 
// impl<RouteId, ChannelId> RouteGraph<RouteId, ChannelId>
// where
//     RouteId: Eq + Hash + Display + Copy,
//     ChannelId: Copy,
// {
//     pub fn topographic_sort(&mut self) {
//         // Set all visited elements to false
//         let visited = &mut (self.visited);
//         visited.clear();
// 
//         let ordering = &mut (self.ordering);
// 
//         let mut index = self.routes.len();
// 
//         for route in self.routes.iter() {
//             if !visited.contains(route) {
//                 // Rust Vecs don't dealloc when you resize them, so this
//                 // is safe to do. Just remember to resize at the end so pushing
//                 // will alloc more!
// 
//                 unsafe {
//                     self.stack.set_len(0);
//                     self.temp_ordering.set_len(0);
//                 }
// 
//                 let stack = &mut (self.stack);
//                 let temp_ordering = &mut (self.temp_ordering);
// 
//                 stack.push(*route);
// 
//                 while let Some(current) = stack.pop() {
//                     if !visited.contains(&current) {
//                         if let Some(node) = self.route_map.get(&current) {
//                             for out in node.out_routes().iter() {
//                                 if !visited.contains(&out.id) {
//                                     stack.push(out.id);
//                                 }
//                             }
//                         }
// 
//                         temp_ordering.push(current);
//                         visited.insert(current);
//                     }
//                 }
// 
//                 for node in temp_ordering.drain(..).rev() {
//                     if self.route_map.contains_key(&node) {
//                         index -= 1;
//                         ordering[index] = node;
//                     } else {
//                         error!("Node {} was targetted, despite not existing in map", node);
//                     }
//                 }
//             }
//         }
// 
//         assert_eq!(ordering.len(), self.routes.len());
// 
//         for (route, ordered) in self.routes.iter_mut().zip(ordering.iter()) {
//             *route = *ordered;
//         }
// 
//         self.sorted = true;
//     }
// 
//     pub fn silence_all_buffers(&mut self) {
//         for route in self.route_map.values_mut() {
//             route.silence_all_buffers();
//         }
//     }
// 
//     fn add_route(&mut self, route: Box<dyn Route<RouteId, ChannelId>>) {
//         let id = route.id();
// 
//         self.routes.push(id);
// 
//         // Increment the ordering, visited and stack so they can be used
//         // for searching without having to alloc memeory
//         self.ordering.push(id);
//         self.temp_ordering.reserve(1);
//         self.visited.reserve(1);
//         self.stack.reserve(1);
//         self.sorted = false;
// 
//         self.route_map.insert(id, route);
//     }
// 
//     pub fn verify(&mut self) -> bool {
//         let visited = &mut (self.visited);
//         visited.clear();
// 
//         for route_id in self.routes.iter() {
//             visited.insert(*route_id);
// 
//             if let Some(route) = self.route_map.get(route_id) {
//                 for out_route in route.out_routes() {
//                     if visited.contains(&out_route.id) {
//                         warn!(
//                             "Route {} has an out route that references visited route {}",
//                             route_id, out_route.id
//                         );
//                         return false;
//                     }
//                 }
//             }
//         }
// 
//         self.sorted = true;
// 
//         true
//     }
// }
