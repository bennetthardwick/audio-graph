extern crate sample;

pub mod graph;
pub mod route;

pub use bufferpool::BufferPoolReference;
pub use graph::*;
pub use route::*;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
