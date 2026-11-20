//! Collection types.
//!
//! `alloc::collections` provides `BTreeMap`, `BTreeSet`, `VecDeque`, etc.,
//! but `HashMap` only exists in `std` because it needs a hasher.  As a
//! `no_std` library, `axstd` provides its own small hash map here so that
//! kernel applications can use `std::collections::HashMap` as usual.

use alloc::vec::Vec;
use core::hash::{Hash, Hasher};

pub use alloc::collections::*;

/// A simple `HashMap` implementation based on separate chaining.
///
/// This is intentionally small but provides the commonly used API.  It uses
/// the FNV-1a hash and grows its bucket array when the load factor exceeds
/// 3/4.
pub struct HashMap<K, V> {
    buckets: Vec<Vec<(K, V)>>,
    len: usize,
}

/// An iterator over the entries of a [`HashMap`].
pub struct Iter<'a, K, V> {
    buckets: core::slice::Iter<'a, Vec<(K, V)>>,
    current: Option<core::slice::Iter<'a, (K, V)>>,
}

/// A mutable iterator over the entries of a [`HashMap`].
pub struct IterMut<'a, K, V> {
    buckets: core::slice::IterMut<'a, Vec<(K, V)>>,
    current: Option<core::slice::IterMut<'a, (K, V)>>,
}

/// An owning iterator over the entries of a [`HashMap`].
pub struct IntoIter<K, V> {
    buckets: alloc::vec::IntoIter<Vec<(K, V)>>,
    current: Option<alloc::vec::IntoIter<(K, V)>>,
}

struct FnvHasher(u64);

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= byte as u64;
            self.0 = self.0.wrapping_mul(0x100_0000_01b3);
        }
    }
}

const INITIAL_BUCKETS: usize = 16;

impl<K, V> HashMap<K, V> {
    /// Creates an empty `HashMap`.
    pub const fn new() -> Self {
        Self {
            buckets: Vec::new(),
            len: 0,
        }
    }

    /// Returns the number of entries in the map.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the map contains no entries.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Removes all entries from the map.
    pub fn clear(&mut self) {
        self.buckets.clear();
        self.len = 0;
    }

    fn hash(key: &K) -> usize
    where
        K: Hash,
    {
        let mut hasher = FnvHasher(0xcbf2_9ce4_8422_2325);
        key.hash(&mut hasher);
        hasher.finish() as usize
    }

    fn ensure_buckets(&mut self) {
        if self.buckets.is_empty() {
            self.buckets
                .resize_with(INITIAL_BUCKETS, Vec::new);
        }
    }

    fn grow(&mut self)
    where
        K: Hash,
    {
        let new_len = self.buckets.len() * 2;
        let old = core::mem::replace(
            &mut self.buckets,
            (0..new_len).map(|_| Vec::new()).collect(),
        );
        for bucket in old {
            for (key, value) in bucket {
                let idx = Self::hash(&key) % new_len;
                self.buckets[idx].push((key, value));
            }
        }
    }
}

impl<K: Hash + Eq, V> HashMap<K, V> {
    /// Inserts a key-value pair, returning the previous value if the key
    /// already existed.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.ensure_buckets();
        let idx = Self::hash(&key) % self.buckets.len();
        for entry in &mut self.buckets[idx] {
            if entry.0 == key {
                return Some(core::mem::replace(&mut entry.1, value));
            }
        }
        self.buckets[idx].push((key, value));
        self.len += 1;
        if self.len > self.buckets.len() * 3 / 4 {
            self.grow();
        }
        None
    }

    /// Returns a reference to the value corresponding to the key.
    pub fn get(&self, key: &K) -> Option<&V> {
        if self.buckets.is_empty() {
            return None;
        }
        let idx = Self::hash(key) % self.buckets.len();
        self.buckets[idx]
            .iter()
            .find_map(|(k, v)| if k == key { Some(v) } else { None })
    }

    /// Returns a mutable reference to the value corresponding to the key.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        if self.buckets.is_empty() {
            return None;
        }
        let idx = Self::hash(key) % self.buckets.len();
        self.buckets[idx]
            .iter_mut()
            .find_map(|(k, v)| if k == key { Some(v) } else { None })
    }

    /// Returns `true` if the map contains the given key.
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// Removes a key from the map, returning the value if it existed.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        if self.buckets.is_empty() {
            return None;
        }
        let idx = Self::hash(key) % self.buckets.len();
        let pos = self.buckets[idx].iter().position(|(k, _)| k == key)?;
        let (_, value) = self.buckets[idx].remove(pos);
        self.len -= 1;
        Some(value)
    }
}

impl<K, V> HashMap<K, V> {
    /// Returns an iterator over the entries of the map.
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            buckets: self.buckets.iter(),
            current: None,
        }
    }

    /// Returns a mutable iterator over the entries of the map.
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut {
            buckets: self.buckets.iter_mut(),
            current: None,
        }
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(current) = &mut self.current {
                if let Some((key, value)) = current.next() {
                    return Some((key, value));
                }
            }
            let bucket = self.buckets.next()?;
            self.current = Some(bucket.iter());
        }
    }
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a mut K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(current) = &mut self.current {
                if let Some((key, value)) = current.next() {
                    return Some((key, value));
                }
            }
            let bucket = self.buckets.next()?;
            self.current = Some(bucket.iter_mut());
        }
    }
}

impl<K, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(current) = &mut self.current {
                if let Some(entry) = current.next() {
                    return Some(entry);
                }
            }
            self.current = Some(self.buckets.next()?.into_iter());
        }
    }
}

impl<K, V> IntoIterator for HashMap<K, V> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            buckets: self.buckets.into_iter(),
            current: None,
        }
    }
}

impl<'a, K, V> IntoIterator for &'a HashMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<K, V> Default for HashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> FromIterator<(K, V)> for HashMap<K, V>
where
    K: Hash + Eq,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut map = Self::new();
        for (key, value) in iter {
            map.insert(key, value);
        }
        map
    }
}
