use generational_arena::{Arena, Index};
use std::mem::MaybeUninit;

pub struct ArenaSplit<'a, T> {
    selected_index: Index,
    arena: &'a mut Arena<T>,
    __type: std::marker::PhantomData<T>,
}

pub fn split_at<'a, T>(
    arena: &'a mut Arena<T>,
    selected: Index,
) -> Option<(&'a mut T, ArenaSplit<'a, T>)> {
    if let Some(value) = arena.get_mut(selected) {
        Some((
            unsafe { (value as *mut T).as_mut().unwrap() },
            ArenaSplit {
                selected_index: selected,
                arena,
                __type: Default::default(),
            },
        ))
    } else {
        None
    }
}

impl<'a, T> ArenaSplit<'a, T> {
    pub fn get_mut(&mut self, index: Index) -> Option<&mut T> {
        if index != self.selected_index {
            self.arena.get_mut(index)
        } else {
            None
        }
    }
}

pub fn insert_with<T>(arena: &mut Arena<T>, create: impl FnOnce(Index) -> T) -> Index {
    unsafe {
        let key = arena.insert(MaybeUninit::<T>::zeroed().assume_init());
        let real_data = create(key);
        let data = arena.get_mut(key).unwrap();
        std::mem::forget(std::mem::replace(data, real_data));
        key
    }
}
