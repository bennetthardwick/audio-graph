use core::cell::RefCell;
use log::error;
use std::rc::Rc;

const BITS_IN_U32: usize = 32;

fn value_of_index(values: &[u32], index: usize) -> Result<bool, ()> {
    let value_index = index / BITS_IN_U32;
    let offset = index % BITS_IN_U32;

    if let Some(v) = values.get(value_index) {
        if offset < 32 {
            Ok(v & (1 << offset) != 0)
        } else {
            Err(())
        }
    } else {
        Err(())
    }
}

fn update_index(values: &mut [u32], index: usize, value: bool) -> Result<(), ()> {
    let value_index = index / BITS_IN_U32;
    let offset = index % BITS_IN_U32;

    if let Some(v) = values.get_mut(value_index) {
        if offset < 32 {
            let mask = 1 << offset;
            if value {
                *v |= mask;
            } else {
                *v &= !mask;
            }
            Ok(())
        } else {
            Err(())
        }
    } else {
        Err(())
    }
}

type Used<T> = Rc<RefCell<Vec<T>>>;

pub struct BufferPool<V: Default> {
    buffer: Vec<V>,
    buffer_size: usize,
    used: Used<u32>,
}

impl<V: Default> BufferPool<V> {
    pub fn new() -> BufferPool<V> {
        BufferPool {
            buffer: vec![],
            buffer_size: 1024,
            used: Rc::new(RefCell::new(vec![])),
        }
    }

    fn find_free_index(&self) -> Result<usize, ()> {
        let mut index = 0;
        let max_index = self.len();

        loop {
            let used = self.used.borrow();
            let used = used.as_slice();

            if let Ok(value) = value_of_index(used, index) {
                if !value {
                    return Ok(index);
                } else {
                    index += 1;

                    if max_index <= index {
                        return Err(());
                    }
                }
            } else {
                return Err(());
            }
        }
    }

    fn set_index_used(&mut self, index: usize) -> Result<(), ()> {
        let mut used = self.used.borrow_mut();
        let used = used.as_mut_slice();
        update_index(used, index, true)
    }

    fn find_free_index_and_use(&mut self) -> Result<usize, ()> {
        if let Ok(index) = self.find_free_index() {
            self.set_index_used(index).unwrap();
            Ok(index)
        } else {
            Err(())
        }
    }

    // pub fn get(&self, id: &BufferPoolId) -> Option<&[V]> {
    //     let input_used = id.used.borrow();
    //     let output_used = self.used.borrow();

    //     if &input_used as *const _ == &output_used as *const _ {
    //         let index = id.index;
    //         let start = index * self.buffer_size;
    //         let end = start + self.buffer_size;

    //         self.buffer.get(start..end)
    //     } else {
    //         None
    //     }
    // }

    // pub fn get_mut(&mut self, id: &BufferPoolId) -> Option<&mut [V]> {
    //     let index = id.index;
    //     let start = index * self.buffer_size;
    //     let end = start + self.buffer_size;

    //     self.buffer.get_mut(start..end)
    // }

    pub fn capacity(&self) -> usize {
        self.buffer.len() / self.buffer_size
    }

    pub fn change_buffer_size(&mut self, new_buffer_size: usize) {
        self.buffer_size = new_buffer_size;
        self.resize(self.len());
    }

    pub fn len(&self) -> usize {
        self.buffer.len() / self.buffer_size
    }

    pub fn reserve(&mut self, additional: usize) {
        self.resize(self.len() + additional);
    }

    // TODO: change this not to resize
    pub fn resize(&mut self, new_len: usize) {
        self.buffer
            .resize_with(new_len * self.buffer_size, || V::default());

        let capacity = self.buffer.len() / self.buffer_size;

        let mut used_capacity = self.used.borrow().len() * BITS_IN_U32;

        while used_capacity < capacity {
            let new_len = self.used.borrow().len() + 1;

            self.used.borrow_mut().resize(new_len, 0);

            used_capacity = self.used.borrow().len() * BITS_IN_U32;
        }
    }

    pub fn get_space<'a, 'b>(&'a mut self) -> Result<BufferPoolReference<'b, V>, ()> {
        self.find_free_index_and_use().and_then(|index| {
            let slice = unsafe {
                std::slice::from_raw_parts_mut(
                    self.buffer.as_mut_ptr().add(index * self.buffer_size),
                    self.buffer_size,
                )
            };

            Ok(BufferPoolReference {
                index,
                used: Rc::clone(&self.used),
                slice,
            })
        })
    }
}

pub struct BufferPoolReference<'a, V> {
    index: usize,
    used: Used<u32>,
    slice: &'a mut [V],
}

impl<V> AsMut<[V]> for BufferPoolReference<'_, V> {
    fn as_mut(&mut self) -> &mut [V] {
        self.slice
    }
}

impl<V> AsRef<[V]> for BufferPoolReference<'_, V> {
    fn as_ref(&self) -> &[V] {
        self.slice
    }
}


impl<V> Drop for BufferPoolReference<'_, V> {
    fn drop(&mut self) {
        let mut used = self.used.borrow_mut();
        let used = used.as_mut_slice();

        if let Err(_) = update_index(used, self.index, false) {
            error!("Unable to free reference for index {}!", self.index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_should_set_bits_and_read() {
        let mut data = vec![0; 3];

        let values = [
            false, true, true, true, false, false, true, true, true, true, false, true, false,
            true, true, true, false, false, true, true, true, true, false, true, false, true, true,
            true, false, false, true, true, true, true, false, true, false, true, true, true,
            false, false, true, true, true, true, false, true, false, true, true, true, false,
            false, true, true, true, true, false, true, false, true, true, true, false, false,
            true, true, true, true, false, true,
        ];

        for (index, _value) in values.iter().enumerate() {
            assert_eq!(value_of_index(&mut data, index).unwrap(), false);
        }

        for (index, value) in values.iter().enumerate() {
            update_index(&mut data, index, *value).unwrap();
        }
        for (index, value) in values.iter().enumerate() {
            assert_eq!(value_of_index(&mut data, index).unwrap(), *value);
        }
    }

    #[test]
    fn it_should_add_capacity() {
        let mut pool: BufferPool<f32> = BufferPool::new();

        assert_eq!(pool.capacity(), 0);

        pool.reserve(1);

        assert_eq!(pool.capacity(), 1);

        pool.reserve(1);

        assert_eq!(pool.capacity(), 2);
    }

    #[test]
    fn it_should_only_be_able_to_get_index_if_capacity() {
        let mut pool: BufferPool<f32> = BufferPool::new();

        assert_eq!(pool.capacity(), 0);

        assert!(pool.get_space().is_err());

        pool.resize(1);

        let index = pool.get_space().unwrap();

        assert!(pool.get_space().is_err());
        assert_eq!(index.index, 0);
    }

    #[test]
    fn it_should_return_space_when_id_is_deallocated() {
        let mut pool: BufferPool<f32> = BufferPool::new();

        assert_eq!(pool.capacity(), 0);
        pool.reserve(1);

        {
            let index = pool.get_space().unwrap();
            assert!(pool.get_space().is_err());
            assert_eq!(index.index, 0);
        }

        assert!(pool.get_space().is_ok());
    }
}
