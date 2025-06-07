use std::{
    hash::{BuildHasherDefault, Hash, Hasher},
    marker::PhantomData,
};

pub trait NoHash: Hash {}

impl NoHash for u8 {}
impl NoHash for u16 {}
impl NoHash for u32 {}
impl NoHash for u64 {}
impl NoHash for usize {}
impl NoHash for i8 {}
impl NoHash for i16 {}
impl NoHash for i32 {}
impl NoHash for i64 {}
impl NoHash for isize {}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoHashHasher<T>(u64, PhantomData<T>);

impl<T: NoHash> Hasher for NoHashHasher<T> {
    fn write(&mut self, _bytes: &[u8]) {
        unreachable!();
    }

    fn write_u8(&mut self, n: u8) {
        self.0 = n as u64;
    }

    fn write_u16(&mut self, n: u16) {
        self.0 = n as u64;
    }

    fn write_u32(&mut self, n: u32) {
        self.0 = n as u64;
    }

    fn write_u64(&mut self, n: u64) {
        self.0 = n;
    }

    fn write_usize(&mut self, n: usize) {
        self.0 = n as u64;
    }

    fn write_i8(&mut self, n: i8) {
        self.0 = n as u64;
    }

    fn write_i16(&mut self, n: i16) {
        self.0 = n as u64;
    }

    fn write_i32(&mut self, n: i32) {
        self.0 = n as u64;
    }

    fn write_i64(&mut self, n: i64) {
        self.0 = n as u64;
    }

    fn write_isize(&mut self, n: isize) {
        self.0 = n as u64;
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

pub type BuildNoHashHasher<T> = BuildHasherDefault<NoHashHasher<T>>;

pub type NoHashMap<K, V> = std::collections::HashMap<K, V, BuildNoHashHasher<K>>;
pub type NoHashSet<T> = std::collections::HashSet<T, BuildNoHashHasher<T>>;
