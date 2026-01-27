//! Hash Map Implementation
//!
//! A BPF hash map provides O(1) average-case key-value lookups.
//! This implementation uses linear probing for collision resolution,
//! which is cache-friendly and suitable for embedded systems.
//!
//! # Memory Layout
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                         Hash Map                                 │
//! ├──────────┬──────────────────────────────────────────────────────┤
//! │ Metadata │                     Buckets                          │
//! │ ┌──────┐ │ ┌─────────┬─────────┬─────────┬─────────┬─────────┐ │
//! │ │count │ │ │ Bucket  │ Bucket  │ Bucket  │ Bucket  │ Bucket  │ │
//! │ │      │ │ │ ┌─────┐ │ ┌─────┐ │ ┌─────┐ │ ┌─────┐ │ ┌─────┐ │ │
//! │ └──────┘ │ │ │state│ │ │state│ │ │state│ │ │state│ │ │state│ │ │
//! │          │ │ │key  │ │ │key  │ │ │key  │ │ │key  │ │ │key  │ │ │
//! │          │ │ │value│ │ │value│ │ │value│ │ │value│ │ │value│ │ │
//! │          │ │ └─────┘ │ └─────┘ │ └─────┘ │ └─────┘ │ └─────┘ │ │
//! │          │ └─────────┴─────────┴─────────┴─────────┴─────────┘ │
//! └──────────┴──────────────────────────────────────────────────────┘
//! ```
//!
//! # Profile Differences
//!
//! | Feature       | Cloud          | Embedded         |
//! |---------------|----------------|------------------|
//! | Allocation    | Dynamic        | Static pool      |
//! | Resize        | Supported      | **Erased**       |
//! | Max entries   | Configurable   | Fixed at init    |
//! | Memory        | Heap           | Pre-allocated    |

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;

use spin::RwLock;

use super::{BpfMap, MapDef, MapError, MapResult, MapType};
use crate::profile::{ActiveProfile, PhysicalProfile};

/// State of a hash bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum BucketState {
    /// Bucket is empty
    Empty = 0,
    /// Bucket contains valid data
    Occupied = 1,
    /// Bucket was deleted (tombstone)
    Deleted = 2,
}

/// A single bucket in the hash map.
#[derive(Clone)]
struct Bucket {
    /// State of this bucket
    state: BucketState,
    /// Key bytes
    key: Vec<u8>,
    /// Value bytes
    value: Vec<u8>,
}

impl Bucket {
    fn empty(key_size: usize, value_size: usize) -> Self {
        Self {
            state: BucketState::Empty,
            key: vec![0u8; key_size],
            value: vec![0u8; value_size],
        }
    }

    fn is_empty(&self) -> bool {
        self.state == BucketState::Empty
    }

    #[allow(dead_code)]
    fn is_occupied(&self) -> bool {
        self.state == BucketState::Occupied
    }

    fn is_deleted(&self) -> bool {
        self.state == BucketState::Deleted
    }

    #[allow(dead_code)]
    fn is_available(&self) -> bool {
        self.state != BucketState::Occupied
    }
}

/// Internal storage for hash map.
struct HashStorage {
    /// Bucket array
    buckets: Vec<Bucket>,
    /// Key size in bytes
    key_size: usize,
    /// Value size in bytes
    value_size: usize,
    /// Number of occupied entries
    count: usize,
    /// Maximum entries (capacity)
    capacity: usize,
}

impl HashStorage {
    fn new(key_size: usize, value_size: usize, capacity: usize) -> Self {
        let buckets = (0..capacity)
            .map(|_| Bucket::empty(key_size, value_size))
            .collect();

        Self {
            buckets,
            key_size,
            value_size,
            count: 0,
            capacity,
        }
    }

    /// Compute hash of a key.
    fn hash(&self, key: &[u8]) -> usize {
        // FNV-1a hash - good distribution for typical BPF workloads
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in key {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash as usize
    }

    /// Find bucket for a key.
    ///
    /// Returns (bucket_index, found) where:
    /// - If found is true, bucket_index is the bucket containing the key
    /// - If found is false, bucket_index is where the key should be inserted
    fn find_bucket(&self, key: &[u8]) -> (usize, bool) {
        let start = self.hash(key) % self.capacity;
        let mut idx = start;
        let mut first_deleted: Option<usize> = None;

        loop {
            let bucket = &self.buckets[idx];

            if bucket.is_empty() {
                // Found empty slot - key doesn't exist
                let insert_idx = first_deleted.unwrap_or(idx);
                return (insert_idx, false);
            }

            if bucket.is_deleted() {
                // Remember first deleted slot for insertion
                if first_deleted.is_none() {
                    first_deleted = Some(idx);
                }
            } else if bucket.key == key {
                // Found the key
                return (idx, true);
            }

            // Linear probing
            idx = (idx + 1) % self.capacity;

            if idx == start {
                // Wrapped around - table is full of non-empty slots
                let insert_idx = first_deleted.unwrap_or(idx);
                return (insert_idx, false);
            }
        }
    }

    fn lookup(&self, key: &[u8]) -> Option<&[u8]> {
        if key.len() != self.key_size {
            return None;
        }

        let (idx, found) = self.find_bucket(key);
        if found {
            Some(&self.buckets[idx].value)
        } else {
            None
        }
    }

    fn update(&mut self, key: &[u8], value: &[u8], flags: u64) -> MapResult<()> {
        if key.len() != self.key_size {
            return Err(MapError::InvalidKey);
        }
        if value.len() != self.value_size {
            return Err(MapError::InvalidValue);
        }

        let (idx, found) = self.find_bucket(key);

        // BPF_NOEXIST (1): fail if key exists
        if flags == 1 && found {
            return Err(MapError::KeyExists);
        }

        // BPF_EXIST (2): fail if key doesn't exist
        if flags == 2 && !found {
            return Err(MapError::KeyNotFound);
        }

        if !found {
            // Check capacity
            if self.count >= self.capacity {
                return Err(MapError::MapFull);
            }
            self.count += 1;
        }

        let bucket = &mut self.buckets[idx];
        bucket.state = BucketState::Occupied;
        bucket.key.copy_from_slice(key);
        bucket.value.copy_from_slice(value);

        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> MapResult<()> {
        if key.len() != self.key_size {
            return Err(MapError::InvalidKey);
        }

        let (idx, found) = self.find_bucket(key);

        if !found {
            return Err(MapError::KeyNotFound);
        }

        self.buckets[idx].state = BucketState::Deleted;
        self.count -= 1;

        Ok(())
    }

    /// Resize the hash map (cloud profile only).
    #[cfg(feature = "cloud-profile")]
    fn resize(&mut self, new_capacity: usize) {
        let old_buckets = core::mem::replace(
            &mut self.buckets,
            (0..new_capacity)
                .map(|_| Bucket::empty(self.key_size, self.value_size))
                .collect(),
        );

        self.capacity = new_capacity;
        self.count = 0;

        // Rehash all existing entries
        for bucket in old_buckets {
            if bucket.is_occupied() {
                // Find new location
                let (idx, _) = self.find_bucket(&bucket.key);
                self.buckets[idx] = bucket;
                self.count += 1;
            }
        }
    }
}

/// Hash map implementation.
///
/// Provides O(1) average-case key-value lookups using linear probing.
pub struct HashMap<P: PhysicalProfile = ActiveProfile> {
    /// Map definition
    def: MapDef,
    /// Storage
    storage: RwLock<HashStorage>,
    /// Profile marker
    _profile: PhantomData<fn() -> P>,
}

impl<P: PhysicalProfile> HashMap<P> {
    /// Create a new hash map.
    ///
    /// # Arguments
    ///
    /// * `def` - Map definition specifying key/value sizes and max entries
    ///
    /// # Errors
    ///
    /// Returns an error if the map definition is invalid.
    pub fn new(def: MapDef) -> MapResult<Self> {
        if def.map_type != MapType::Hash {
            return Err(MapError::InvalidMapType);
        }

        if def.key_size == 0 {
            return Err(MapError::InvalidKey);
        }

        if def.value_size == 0 {
            return Err(MapError::InvalidValue);
        }

        if def.max_entries == 0 {
            return Err(MapError::InvalidValue);
        }

        // Check memory budget for embedded profile
        #[cfg(feature = "embedded-profile")]
        {
            use crate::profile::MemoryStrategy;
            let budget = <P::MemoryStrategy as MemoryStrategy>::MEMORY_BUDGET;
            if budget > 0 && def.total_size() > budget {
                return Err(MapError::OutOfMemory);
            }
        }

        let storage = HashStorage::new(
            def.key_size as usize,
            def.value_size as usize,
            def.max_entries as usize,
        );

        Ok(Self {
            def,
            storage: RwLock::new(storage),
            _profile: PhantomData,
        })
    }

    /// Create a hash map with specified sizes.
    pub fn with_sizes(key_size: u32, value_size: u32, max_entries: u32) -> MapResult<Self> {
        let def = MapDef::new(MapType::Hash, key_size, value_size, max_entries);
        Self::new(def)
    }

    /// Get the number of entries in the map.
    pub fn len(&self) -> usize {
        self.storage.read().count
    }

    /// Check if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the capacity of the map.
    pub fn capacity(&self) -> usize {
        self.storage.read().capacity
    }
}

impl<P: PhysicalProfile> BpfMap<P> for HashMap<P> {
    fn lookup(&self, key: &[u8]) -> Option<Vec<u8>> {
        let guard = self.storage.read();
        guard.lookup(key).map(|v| v.to_vec())
    }

    fn update(&self, key: &[u8], value: &[u8], flags: u64) -> MapResult<()> {
        let mut guard = self.storage.write();
        guard.update(key, value, flags)
    }

    fn delete(&self, key: &[u8]) -> MapResult<()> {
        let mut guard = self.storage.write();
        guard.delete(key)
    }

    fn def(&self) -> &MapDef {
        &self.def
    }

    unsafe fn lookup_ptr(&self, key: &[u8]) -> Option<*mut u8> {
        let guard = self.storage.read();
        let slice = guard.lookup(key)?;
        Some(slice.as_ptr() as *mut u8)
    }

    #[cfg(feature = "cloud-profile")]
    fn resize(&mut self, new_max_entries: u32) -> MapResult<()> {
        let mut guard = self.storage.write();

        // Check that new size can hold existing entries
        if (new_max_entries as usize) < guard.count {
            return Err(MapError::InvalidValue);
        }

        guard.resize(new_max_entries as usize);
        self.def.max_entries = new_max_entries;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_hash_map() {
        let map = HashMap::<ActiveProfile>::with_sizes(4, 8, 100).expect("create map");
        assert_eq!(map.def().max_entries, 100);
        assert_eq!(map.def().key_size, 4);
        assert_eq!(map.def().value_size, 8);
        assert!(map.is_empty());
    }

    #[test]
    fn hash_map_operations() {
        let map = HashMap::<ActiveProfile>::with_sizes(4, 8, 100).expect("create map");

        // Insert
        let key = 42u32.to_ne_bytes();
        let value = 123u64.to_ne_bytes();
        map.update(&key, &value, 0).expect("insert");
        assert_eq!(map.len(), 1);

        // Lookup
        let result = map.lookup(&key).expect("lookup");
        assert_eq!(result, value);

        // Update existing
        let new_value = 456u64.to_ne_bytes();
        map.update(&key, &new_value, 0).expect("update");
        let result = map.lookup(&key).expect("lookup after update");
        assert_eq!(result, new_value);
        assert_eq!(map.len(), 1);

        // Delete
        map.delete(&key).expect("delete");
        assert!(map.lookup(&key).is_none());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn hash_map_noexist_flag() {
        let map = HashMap::<ActiveProfile>::with_sizes(4, 8, 100).expect("create map");

        let key = 1u32.to_ne_bytes();
        let value = [0u8; 8];

        // First insert should succeed
        map.update(&key, &value, 1).expect("first insert");

        // Second insert with NOEXIST should fail
        let result = map.update(&key, &value, 1);
        assert!(matches!(result, Err(MapError::KeyExists)));
    }

    #[test]
    fn hash_map_exist_flag() {
        let map = HashMap::<ActiveProfile>::with_sizes(4, 8, 100).expect("create map");

        let key = 1u32.to_ne_bytes();
        let value = [0u8; 8];

        // Update with EXIST flag on non-existent key should fail
        let result = map.update(&key, &value, 2);
        assert!(matches!(result, Err(MapError::KeyNotFound)));

        // Insert first
        map.update(&key, &value, 0).expect("insert");

        // Now update with EXIST should succeed
        map.update(&key, &value, 2).expect("update existing");
    }

    #[test]
    fn hash_map_many_entries() {
        let map = HashMap::<ActiveProfile>::with_sizes(4, 4, 1000).expect("create map");

        // Insert many entries
        for i in 0u32..500 {
            let key = i.to_ne_bytes();
            let value = (i * 2).to_ne_bytes();
            map.update(&key, &value, 0).expect("insert");
        }

        assert_eq!(map.len(), 500);

        // Verify all entries
        for i in 0u32..500 {
            let key = i.to_ne_bytes();
            let result = map.lookup(&key).expect("lookup");
            let expected = (i * 2).to_ne_bytes();
            assert_eq!(result, expected);
        }

        // Delete half
        for i in 0u32..250 {
            let key = i.to_ne_bytes();
            map.delete(&key).expect("delete");
        }

        assert_eq!(map.len(), 250);

        // Verify deleted entries are gone
        for i in 0u32..250 {
            let key = i.to_ne_bytes();
            assert!(map.lookup(&key).is_none());
        }

        // Verify remaining entries
        for i in 250u32..500 {
            let key = i.to_ne_bytes();
            assert!(map.lookup(&key).is_some());
        }
    }

    #[test]
    fn hash_map_full() {
        let map = HashMap::<ActiveProfile>::with_sizes(4, 4, 10).expect("create map");

        // Fill the map
        for i in 0u32..10 {
            let key = i.to_ne_bytes();
            let value = i.to_ne_bytes();
            map.update(&key, &value, 0).expect("insert");
        }

        // Next insert should fail
        let key = 100u32.to_ne_bytes();
        let value = 100u32.to_ne_bytes();
        let result = map.update(&key, &value, 0);
        assert!(matches!(result, Err(MapError::MapFull)));
    }

    #[test]
    fn hash_map_reuse_deleted() {
        let map = HashMap::<ActiveProfile>::with_sizes(4, 4, 10).expect("create map");

        // Fill the map
        for i in 0u32..10 {
            let key = i.to_ne_bytes();
            let value = i.to_ne_bytes();
            map.update(&key, &value, 0).expect("insert");
        }

        // Delete one entry
        let key = 5u32.to_ne_bytes();
        map.delete(&key).expect("delete");

        // Should be able to insert a new entry
        let new_key = 100u32.to_ne_bytes();
        let new_value = 100u32.to_ne_bytes();
        map.update(&new_key, &new_value, 0)
            .expect("insert into deleted slot");
    }

    #[test]
    fn hash_map_invalid_sizes() {
        // Zero key size
        let result = HashMap::<ActiveProfile>::with_sizes(0, 8, 100);
        assert!(matches!(result, Err(MapError::InvalidKey)));

        // Zero value size
        let result = HashMap::<ActiveProfile>::with_sizes(4, 0, 100);
        assert!(matches!(result, Err(MapError::InvalidValue)));

        // Zero entries
        let result = HashMap::<ActiveProfile>::with_sizes(4, 8, 0);
        assert!(matches!(result, Err(MapError::InvalidValue)));
    }

    #[test]
    fn hash_map_wrong_key_size() {
        let map = HashMap::<ActiveProfile>::with_sizes(4, 8, 100).expect("create map");

        // Wrong key size on lookup
        let bad_key = [1u8, 2, 3]; // 3 bytes instead of 4
        assert!(map.lookup(&bad_key).is_none());

        // Wrong key size on update
        let value = [0u8; 8];
        assert!(matches!(
            map.update(&bad_key, &value, 0),
            Err(MapError::InvalidKey)
        ));
    }

    #[test]
    fn hash_map_wrong_value_size() {
        let map = HashMap::<ActiveProfile>::with_sizes(4, 8, 100).expect("create map");

        let key = 1u32.to_ne_bytes();
        let bad_value = [1u8, 2, 3]; // 3 bytes instead of 8

        assert!(matches!(
            map.update(&key, &bad_value, 0),
            Err(MapError::InvalidValue)
        ));
    }

    #[cfg(feature = "cloud-profile")]
    #[test]
    fn hash_map_resize() {
        let mut map = HashMap::<ActiveProfile>::with_sizes(4, 4, 10).expect("create map");

        // Insert some entries
        for i in 0u32..5 {
            let key = i.to_ne_bytes();
            let value = (i * 10).to_ne_bytes();
            map.update(&key, &value, 0).expect("insert");
        }

        // Resize to larger
        map.resize(20).expect("resize");
        assert_eq!(map.capacity(), 20);

        // Verify entries still exist
        for i in 0u32..5 {
            let key = i.to_ne_bytes();
            let result = map.lookup(&key).expect("lookup after resize");
            let expected = (i * 10).to_ne_bytes();
            assert_eq!(result, expected);
        }

        // Can now insert more entries
        for i in 10u32..20 {
            let key = i.to_ne_bytes();
            let value = i.to_ne_bytes();
            map.update(&key, &value, 0).expect("insert after resize");
        }
    }
}
