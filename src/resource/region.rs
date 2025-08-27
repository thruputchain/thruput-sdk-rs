use super::keypath::*;
use super::handle::*;

pub struct Region {
    name: &'static str,
    i: u64
}

impl Region {
    pub fn new(name: &'static str) -> Self {
        Self { name, i: 0 }
    }

    pub fn alloc<T>(&mut self) -> Handle<T> {
        let mut kp = Keypath::root();
        kp.push_item(self.name.as_bytes());
        kp.push_index(self.i);

        self.i += 1;

        kp.into()
    }
}
