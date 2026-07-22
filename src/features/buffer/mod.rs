//! Fixed-capacity Vec and supporting types for no_std.

use core::ops::Index;

pub struct Vec<T: Copy> {
    items: [T; 256],
    len: usize,
}

impl<T: Copy + ConstDefault> Vec<T> {
    pub fn new() -> Self {
        Vec {
            items: [T::CONST_DEFAULT; 256],
            len: 0,
        }
    }

    pub fn push(&mut self, item: T) {
        if self.len < 256 {
            self.items[self.len] = item;
            self.len += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> core::slice::Iter<T> {
        self.items[..self.len].iter()
    }

    pub fn extend_from_slice(&mut self, other: &[T]) {
        for &item in other {
            self.push(item);
        }
    }
}

impl<T: Copy + ConstDefault> Index<usize> for Vec<T> {
    type Output = T;
    fn index(&self, i: usize) -> &T {
        &self.items[i]
    }
}

pub trait ConstDefault: Copy {
    const CONST_DEFAULT: Self;
}

impl ConstDefault for u8 {
    const CONST_DEFAULT: Self = 0;
}

impl ConstDefault for usize {
    const CONST_DEFAULT: Self = 0;
}

impl ConstDefault for [u8; 64] {
    const CONST_DEFAULT: Self = [0u8; 64];
}

impl ConstDefault for [u8; 128] {
    const CONST_DEFAULT: Self = [0u8; 128];
}

impl Vec<u8> {
    pub fn as_slice(&self) -> &[u8] {
        &self.items[..self.len]
    }
}

pub fn line_to_vec(line: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(line);
    v
}
