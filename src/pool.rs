pub trait Pool<T>
{
    fn new(capacity: usize) -> Self;
    fn capacity(&self) -> usize;
    fn insert(&mut self, value: T) -> PoolKey;
    fn get(&self, key: &PoolKey) -> Option<&T>;
    fn get_mut(&mut self, key: &PoolKey) -> Option<&mut T>;
    fn take(&mut self, key: &PoolKey) -> Option<T>;
    fn delete(&mut self, key: &PoolKey);
}


#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PoolKey
{
    index: usize,
    generation: usize,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
struct PoolItem<T>
{
    generation: usize,
    data: Option<T>,
}


impl<T> PoolItem<T>
{
    fn new() -> Self
    {
        PoolItem {
            generation: 0,
            data: None,
        }
    }

    // ====-====-====-====-====-==== //

    fn set(&mut self, value: T) -> usize
    {
        assert!(self.data.is_none());

        self.data = Some(value);
        self.generation += 1;

        return self.generation;
    }

    fn get(&self) -> Option<&T>
    {
        if let Some(ref data) = self.data { Some(data) }
        else                              { None }
    }

    fn get_mut(&mut self) -> Option<&mut T>
    {
        if let Some(ref mut data) = self.data { Some(data) }
        else                                  { None }
    }

    fn clear(&mut self)
    {
        self.data = None;
    }

    fn is_empty(&self) -> bool
    {
        return self.data.is_none();
    }

    fn take(&mut self) -> Option<T>
    {
        return self.data.take();
    }
}

#[cfg(test)]
mod pool_item
{
    mod default
    {
        use super::super::PoolItem;

        #[test]
        fn default_makes_sense()
        {
            let val: PoolItem<i32> = Default::default();

            assert_eq!(val.generation, 0);
            assert!(val.data.is_none());
        }
    }

    mod set
    {
        use super::super::PoolItem;

        #[test]
        fn increments_generation()
        {
            let mut val: PoolItem<i32> = Default::default();
            let test_gen = val.generation + 1;

            val.set(100);

            assert_eq!(val.generation, test_gen);
            assert!(val.data.is_some());

            let inner = val.data.unwrap();
            assert_eq!(inner, 100);
        }

        #[test]
        #[should_panic]
        fn panics_if_overwriting_value()
        {
            let mut val: PoolItem<i32> = Default::default();

            val.set(100);
            val.set(200);
        }
    }

    mod get
    {
        use super::super::PoolItem;

        #[test]
        fn returns_none_if_empty()
        {
            let val: PoolItem<i32> = Default::default();
            assert!(val.get().is_none());
        }

        #[test]
        fn returns_some_if_not_empty()
        {
            let mut val: PoolItem<i32> = Default::default();
            val.set(100);
            assert!(val.get().is_some());
        }
    }

    mod get_mut
    {
        use super::super::PoolItem;

        #[test]
        fn returns_none_if_empty()
        {
            let mut val: PoolItem<i32> = Default::default();
            assert!(val.get_mut().is_none());
        }

        #[test]
        fn returns_some_if_not_empty()
        {
            let mut val: PoolItem<i32> = Default::default();
            val.set(100);
            assert!(val.get_mut().is_some());
        }
    }

    mod clear
    {
        use super::super::PoolItem;

        #[test]
        fn sets_contents_to_none_without_advancing_generation()
        {
            let mut val: PoolItem<i32> = Default::default();
            val.set(100);

            let generation = val.generation;
            val.clear();

            assert_eq!(generation, val.generation);
            assert!(val.data.is_none());
        }
    }
}


// ===-===-===-===-===-===-===-===-===-===-===-===-=== //

/// The default ObjectPool implementation.
///
/// Allocation of specified capacity happens completely upfront, and the pool cannot be resized.
///
/// Items are eagerly dropped when [`deleted`], so destructors run asap.
///
/// See [`Pool`] implementation for more information.
///
/// [`deleted`]: struct.ObjectPool.delete
/// [`Pool`]: trait.Pool.html
///
/// ```rust
/// # use std::error::Error;
/// #
/// # fn main() -> Result<(), Box<dyn Error>> {
/// use spool::{ ObjectPool, Pool };
///
/// //Pool allocates _once_ upfront, with given capacity.
/// let mut pool = ObjectPool::new(3);
///
/// let key1 = pool.insert(1);
/// let key2 = pool.insert(2);
/// let key3 = pool.insert(3);
///
/// //Over capacity! This panics!
/// //let key4 = pool.insert(404);
///
/// pool.delete(&key2);
///
/// //All is well.
/// let key4 = pool.insert(404);
/// #
/// #     Ok(())
/// # }
/// ```

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ObjectPool<T>
{
    count: usize,
    next: usize,
    free: Vec<usize>,
    data: Vec<PoolItem<T>>,
}

impl<T> Pool<T> for ObjectPool<T>
{
    /// Returns a new, empty pool. Preallocated with specified capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use spool::{ ObjectPool, Pool };
    ///
    /// let pool: ObjectPool<i32> = ObjectPool::new(10);
    /// assert_eq!(pool.capacity(), 10);
    /// ```
    fn new(capacity: usize) -> Self
    {
        Self {
            count: 0,
            next: 0,
            free: Vec::new(),
            data: {
                let mut data = Vec::with_capacity(capacity);
                data.resize_with(capacity, PoolItem::new);
                data
            }
        }
    }

    // ====-====-====-====-====-==== //

    /// Returns the maximum capacity of the pool.
    ///
    /// # Examples
    ///
    /// ```
    /// use spool::{ ObjectPool, Pool };
    ///
    /// let pool: ObjectPool<i32> = ObjectPool::new(10);
    /// assert_eq!(pool.capacity(), 10);
    /// ```
    fn capacity(&self) -> usize { self.data.capacity() }

    // ====-====-====-====-====-==== //

    /// Returns a [`PoolKey`] corresponding to the inserted item.
    ///
    /// [`PoolKey`]: struct.PoolKey.html
    ///
    /// # Panics
    ///
    /// This function panics if pool is full.
    ///
    /// # Examples
    ///
    /// ```
    /// use spool::{ ObjectPool, Pool };
    ///
    /// let mut pool = ObjectPool::new(10);
    /// let key = pool.insert("Howdy!");
    /// ```
    fn insert(&mut self, value: T) -> PoolKey
    {
        let index =
            if self.free.len() > 0
            {
                self.free.pop().unwrap()
            }
            else if self.next < self.data.capacity()
            {
                let index = self.next;
                self.next += 1;
                index
            }
            else
            {
                // TODO: Result with an error?
                panic!();
            };

        let generation = unsafe {
            self.data.get_unchecked_mut(index).set(value)
        };
        self.count += 1;

        return PoolKey {
            index,
            generation,
        };
    }

    /// Retrieves an Option<&T> corresponding to the [`PoolKey`] referenced.
    ///
    /// [`PoolKey`]: struct.PoolKey.html
    ///
    /// # Examples
    ///
    /// ```
    /// use spool::{ ObjectPool, Pool };
    ///
    /// let mut pool = ObjectPool::new(10);
    ///
    /// let key1 = pool.insert("I am going to be removed!");
    /// let key2 = pool.insert("I am going to remain!");
    ///
    /// pool.delete(&key1);
    ///
    /// assert!(pool.get(&key1).is_none());
    /// assert!(pool.get(&key2).is_some());
    /// ```
    fn get(&self, key: &PoolKey) -> Option<&T>
    {
        if key.index >= self.data.capacity() { return None; }
        else
        {
            let entry = unsafe {
                self.data.get_unchecked(key.index)
            };

            if entry.generation != key.generation || entry.is_empty() { return None; }
            else { return entry.get(); }
        }
    }

    /// Retrieves an Option<&T> corresponding to the [`PoolKey`] referenced.
    ///
    /// [`PoolKey`]: struct.PoolKey.html
    ///
    /// # Examples
    ///
    /// ```
    /// use spool::{ ObjectPool, Pool };
    ///
    /// let mut pool = ObjectPool::new(10);
    ///
    /// let key1 = pool.insert("I am going to be removed!");
    /// let key2 = pool.insert("I am going to remain!");
    ///
    /// pool.delete(&key1);
    ///
    /// assert!(pool.get_mut(&key1).is_none());
    /// assert!(pool.get_mut(&key2).is_some());
    /// ```
    fn get_mut(&mut self, key: &PoolKey) -> Option<&mut T>
    {
        if key.index >= self.data.capacity() { return None; }
        else
        {
            let entry = unsafe {
                self.data.get_unchecked_mut(key.index)
            };

            if entry.generation != key.generation || entry.is_empty() { return None; }
            else { return entry.get_mut(); }
        }
    }

    /// Extracts an Option<T> corresponding to the [`PoolKey`] referenced.
    /// When an entry is been [`taken`] it is removed from the pool.
    ///
    /// [`PoolKey`]: struct.PoolKey.html
    /// [`taken`]: #method.take
    ///
    /// # Examples
    ///
    /// ```
    /// use spool::{ ObjectPool, Pool };
    ///
    /// let mut pool = ObjectPool::new(10);
    ///
    /// let key = pool.insert("Take me!");
    ///
    /// assert!(pool.take(&key).is_some());
    /// assert!(pool.get(&key).is_none());
    /// ```
    fn take(&mut self, key: &PoolKey) -> Option<T>
    {
        if key.index >= self.data.capacity() { return None; }
        else
        {
            let entry = unsafe {
                self.data.get_unchecked_mut(key.index)
            };

            if entry.generation != key.generation || entry.is_empty() { return None; }

            self.count -= 1;
            self.free.push(key.index);
            return entry.take();
        }
    }

    /// Deletes an entry corresponding to the [`PoolKey`] referenced.
    /// When an entry is been [`deleted`] it is removed, however it will not be returned.
    ///
    /// [`PoolKey`]: struct.PoolKey.html
    /// [`take`]: #method.take
    /// [`deleted`]: #method.delete
    ///
    /// # Examples
    ///
    /// ```
    /// use spool::{ ObjectPool, Pool };
    ///
    /// let mut pool = ObjectPool::new(10);
    ///
    /// let key = pool.insert("Delete me!");
    ///
    /// pool.delete(&key);
    ///
    /// assert!(pool.get(&key).is_none());
    /// ```
    fn delete(&mut self, key: &PoolKey)
    {
        if key.index >= self.data.capacity() { return; }
        else
        {
            let entry = unsafe {
                self.data.get_unchecked_mut(key.index)
            };

            if entry.generation != key.generation || entry.is_empty() { return; }

            entry.clear();
            self.count -= 1;
            self.free.push(key.index);
        }
    }
}


#[cfg(test)]
mod vector_backed_pool
{
    mod new
    {
        use super::super::{
            Pool,
            ObjectPool,
        };

        #[test]
        fn correctly_initializes_a_pool()
        {
            let pool: ObjectPool<i32> = ObjectPool::new(10);

            assert_eq!(pool.capacity(), 10);
            assert_eq!(pool.count, 0);
            assert_eq!(pool.next, 0);
            assert_eq!(pool.free.len(), 0);
            assert_eq!(pool.data.len(), pool.capacity());
        }
    }

    mod insert
    {
        use super::super::{
            Pool,
            ObjectPool,
        };

        #[test]
        fn correctly_updates_pool_state()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            assert!(pool.data[key.index].data.is_some());
            assert_eq!(pool.data[key.index].data.unwrap(), 100);
            assert_eq!(pool.capacity(), 10);
            assert_eq!(pool.count, 1);
            assert_eq!(pool.next, 1);
            assert_eq!(pool.free.len(), 0);
            assert_eq!(pool.data.len(), pool.capacity());

            pool.delete(&key);

            let key = pool.insert(200);

            assert!(pool.data[key.index].data.is_some());
            assert_eq!(pool.data[key.index].data.unwrap(), 200);
            assert_eq!(pool.capacity(), 10);
            assert_eq!(pool.count, 1);
            assert_eq!(pool.next, 1);
            assert_eq!(pool.free.len(), 0);
            assert_eq!(pool.data.len(), pool.capacity());
        }

        #[test]
        fn returns_valid_key_pointing_to_expected_data()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            assert_eq!(key.index, 0, "Expected index of first inserted element to be 0.");
            assert_eq!(key.generation, 1, "Expected generation of first inserted element to be 1.");
            assert_eq!(pool.data[key.index].generation, key.generation, "Expected generation of stored item to match key.");
            assert!(pool.data[key.index].data.is_some(), "Expected data at key index to be Some().");
            assert_eq!(*pool.data[key.index].data.as_ref().unwrap(), 100, "Expected value at key index to be 100.");
        }

        #[test]
        #[should_panic]
        fn should_panic_if_full()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            for i in 0..10 { pool.insert(i); }

            pool.insert(100);
        }
    }

    mod get
    {
        use super::super::{
            Pool,
            PoolKey,
            ObjectPool,
        };

        #[test]
        fn returns_some_reference_to_entry_specified()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key1 = pool.insert(100);
            let key2 = pool.insert(200);
            let key3 = pool.insert(300);

            // Out of order 'cause :shrug:
            let get2 = pool.get(&key2);
            let get1 = pool.get(&key1);
            let get3 = pool.get(&key3);

            assert!(get1.is_some());
            assert_eq!(*get1.unwrap(), 100);

            assert!(get2.is_some());
            assert_eq!(*get2.unwrap(), 200);

            assert!(get3.is_some());
            assert_eq!(*get3.unwrap(), 300);
        }

        #[test]
        fn returns_none_if_key_has_invalid_index()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            pool.insert(100);

            let key_at_cap = PoolKey { index: 10, generation: 0 };
            let get_at_cap = pool.get(&key_at_cap);
            assert!(get_at_cap.is_none());

            let key_over_cap = PoolKey { index: 1000, generation: 0 };
            let get_over_cap = pool.get(&key_over_cap);
            assert!(get_over_cap.is_none());
        }

        #[test]
        fn returns_none_if_generation_mismatch()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            pool.data[key.index].generation = 42;

            let get = pool.get(&key);
            assert!(get.is_none());
        }

        #[test]
        fn returns_none_if_data_is_none()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            pool.data[key.index].data = None;

            let get = pool.get(&key);
            assert!(get.is_none());
        }
    }

    mod get_mut
    {
        use super::super::{
            Pool,
            PoolKey,
            ObjectPool,
        };

        #[test]
        fn returns_some_reference_to_entry_specified()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key1 = pool.insert(100);
            let key2 = pool.insert(200);
            let key3 = pool.insert(300);

            // Out of order 'cause :shrug:
            let get2 = pool.get_mut(&key2);
            assert!(get2.is_some());
            assert_eq!(*get2.unwrap(), 200);

            let get1 = pool.get_mut(&key1);
            assert!(get1.is_some());
            assert_eq!(*get1.unwrap(), 100);

            let get3 = pool.get_mut(&key3);
            assert!(get3.is_some());
            assert_eq!(*get3.unwrap(), 300);
        }

        #[test]
        fn returns_none_if_key_has_invalid_index()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            pool.insert(100);

            let key_at_cap = PoolKey { index: 10, generation: 0 };
            let get_at_cap = pool.get_mut(&key_at_cap);
            assert!(get_at_cap.is_none());

            let key_over_cap = PoolKey { index: 1000, generation: 0 };
            let get_over_cap = pool.get_mut(&key_over_cap);
            assert!(get_over_cap.is_none());
        }

        #[test]
        fn returns_none_if_generation_mismatch()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            pool.data[key.index].generation = 42;

            let get = pool.get_mut(&key);
            assert!(get.is_none());
        }

        #[test]
        fn returns_none_if_data_is_none()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            pool.data[key.index].data = None;

            let get = pool.get_mut(&key);
            assert!(get.is_none());
        }
    }

    mod take
    {
        use super::super::{
            Pool,
            PoolKey,
            ObjectPool,
        };

        #[test]
        fn replaces_item_with_none_and_pushes_index_to_free()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            let old_count = pool.count;
            let old_free_len = pool.free.len();

            let taken = pool.take(&key);

            assert!(pool.data[key.index].data.is_none(), "Expected data to be set to None.");
            assert_eq!(pool.data[key.index].generation, key.generation, "Expected generation to remain unchanged.");
            assert_eq!(pool.count, old_count - 1, "Expected count to be decremented.");
            assert_eq!(pool.free.len(), old_free_len + 1, "Expected free list length to be incremented.");

            let free_item = pool.free.last();
            assert!(free_item.is_some());
            assert_eq!(*free_item.unwrap(), key.index, "Expected key index to be most recent addition to free list.");

            assert!(taken.is_some());
            assert_eq!(taken.unwrap(), 100, "Expected taken value to match what was inserted.");
        }

        #[test]
        fn returns_none_if_key_has_invalid_index()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            let old_count = pool.count;
            let old_free_len = pool.free.len();

            let key_at_cap = PoolKey { index: 1000, generation: 0 };
            let taken = pool.take(&key_at_cap);

            assert!(pool.data[key.index].data.is_some(), "Expected data to be unchanged.");
            assert!(taken.is_none(), "Expected taken value to be None.");
            assert_eq!(pool.data[key.index].generation, key.generation, "Expected generation to remain unchanged.");
            assert_eq!(pool.count, old_count, "Expected count to be unchanged.");
            assert_eq!(pool.free.len(), old_free_len, "Expected free list length to be unchanged.");


            let key_over_cap = PoolKey { index: 1000, generation: 0 };
            let taken = pool.take(&key_over_cap);

            assert!(pool.data[key.index].data.is_some(), "Expected data to be unchanged.");
            assert!(taken.is_none(), "Expected taken value to be None.");
            assert_eq!(pool.data[key.index].generation, key.generation, "Expected generation to remain unchanged.");
            assert_eq!(pool.count, old_count, "Expected count to be unchanged.");
            assert_eq!(pool.free.len(), old_free_len, "Expected free list length to be unchanged.");
        }

        #[test]
        fn returns_none_if_generation_mismatch()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            let old_count = pool.count;
            let old_free_len = pool.free.len();

            let mut bad_key = key;
            bad_key.generation = 100;
            let taken = pool.take(&bad_key);

            assert!(taken.is_none(), "Expected taken value to be None.");
            assert!(pool.data[key.index].data.is_some(), "Expected data to be unchanged.");
            assert_eq!(pool.data[key.index].generation, key.generation, "Expected generation to remain unchanged.");
            assert_eq!(pool.count, old_count, "Expected count to be unchanged.");
            assert_eq!(pool.free.len(), old_free_len, "Expected free list length to be unchanged.");
        }

        #[test]
        fn returns_none_if_data_is_none()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            let old_count = pool.count;
            let old_free_len = pool.free.len();

            pool.data[key.index].data = None;
            let taken = pool.take(&key);

            assert!(taken.is_none(), "Expected taken value to be None.");
            assert_eq!(pool.data[key.index].generation, key.generation, "Expected generation to remain unchanged.");
            assert_eq!(pool.count, old_count, "Expected count to be unchanged.");
            assert_eq!(pool.free.len(), old_free_len, "Expected free list length to be unchanged.");
        }
    }

    mod delete
    {
        use super::super::{
            Pool,
            PoolKey,
            ObjectPool,
        };

        #[test]
        fn replaces_item_with_none_and_pushes_index_to_free()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            let old_count = pool.count;
            let old_free_len = pool.free.len();

            pool.delete(&key);

            assert!(pool.data[key.index].data.is_none(), "Expected data to be set to None.");
            assert_eq!(pool.data[key.index].generation, key.generation, "Expected generation to remain unchanged.");
            assert_eq!(pool.count, old_count - 1, "Expected count to be decremented.");
            assert_eq!(pool.free.len(), old_free_len + 1, "Expected free list length to be incremented.");

            let free_item = pool.free.last();
            assert!(free_item.is_some());
            assert_eq!(*free_item.unwrap(), key.index, "Expected key index to be most recent addition to free list.");
        }

        #[test]
        fn does_nothing_if_key_has_invalid_index()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            let old_count = pool.count;
            let old_free_len = pool.free.len();

            let key_at_cap = PoolKey { index: 1000, generation: 0 };
            pool.delete(&key_at_cap);

            assert!(pool.data[key.index].data.is_some(), "Expected data to be unchanged.");
            assert_eq!(pool.data[key.index].generation, key.generation, "Expected generation to remain unchanged.");
            assert_eq!(pool.count, old_count, "Expected count to be unchanged.");
            assert_eq!(pool.free.len(), old_free_len, "Expected free list length to be unchanged.");


            let key_over_cap = PoolKey { index: 1000, generation: 0 };
            pool.delete(&key_over_cap);

            assert!(pool.data[key.index].data.is_some(), "Expected data to be unchanged.");
            assert_eq!(pool.data[key.index].generation, key.generation, "Expected generation to remain unchanged.");
            assert_eq!(pool.count, old_count, "Expected count to be unchanged.");
            assert_eq!(pool.free.len(), old_free_len, "Expected free list length to be unchanged.");
        }

        #[test]
        fn returns_none_if_generation_mismatch()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            let old_count = pool.count;
            let old_free_len = pool.free.len();

            let mut bad_key = key;
            bad_key.generation = 100;
            pool.delete(&bad_key);

            assert!(pool.data[key.index].data.is_some(), "Expected data to be unchanged.");
            assert_eq!(pool.data[key.index].generation, key.generation, "Expected generation to remain unchanged.");
            assert_eq!(pool.count, old_count, "Expected count to be unchanged.");
            assert_eq!(pool.free.len(), old_free_len, "Expected free list length to be unchanged.");
        }

        #[test]
        fn returns_none_if_data_is_none()
        {
            let mut pool: ObjectPool<i32> = ObjectPool::new(10);
            let key = pool.insert(100);

            let old_count = pool.count;
            let old_free_len = pool.free.len();

            pool.data[key.index].data = None;
            pool.delete(&key);

            assert_eq!(pool.data[key.index].generation, key.generation, "Expected generation to remain unchanged.");
            assert_eq!(pool.count, old_count, "Expected count to be unchanged.");
            assert_eq!(pool.free.len(), old_free_len, "Expected free list length to be unchanged.");
        }
    }
}
