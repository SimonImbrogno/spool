mod pool;
pub use pool::{ Pool, PoolKey, VectorBackedPool };

pub fn create_default_pool<T>(capacity: usize) -> impl Pool<T>
{
    return VectorBackedPool::new(capacity);
}
