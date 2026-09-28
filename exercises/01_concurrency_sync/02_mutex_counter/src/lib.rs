//! # Mutex Shared State
//!
//! In this exercise, you will use `Arc<Mutex<T>>` to safely share and modify data between multiple threads.
//!
//! ## Concepts
//! - `Mutex<T>` mutex protects shared data
//! - `Arc<T>` atomic reference counting enables cross-thread sharing
//! - `lock()` acquires the lock and accesses data
use std::sync::{Arc, Mutex};
use std::thread;
/// Increment a counter concurrently using `n_threads` threads.
/// Each thread increments the counter `count_per_thread` times.
/// Returns the final counter value.
///
/// Hint: Use `Arc<Mutex<usize>>` as the shared counter.
pub fn concurrent_counter(n_threads: usize, count_per_thread: usize) -> usize {
    // Create shared counter wrapped by Arc + Mutex
    let counter = Arc::new(Mutex::new(0));
    let mut handles = Vec::new();

    for _ in 0..n_threads {
        let counter_clone = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            for _ in 0..count_per_thread {
                let mut val = counter_clone.lock().unwrap();
                *val += 1;
            }
        });
        handles.push(handle);
    }

    // Wait for all threads finish
    for h in handles {
        h.join().unwrap();
    }

    // Get final value
    let final_count = counter.lock().unwrap();
    *final_count
}

/// Add elements to a shared vector concurrently using multiple threads.
/// Each thread pushes its own id (0..n_threads) to the vector.
/// Returns the sorted vector.
///
/// Hint: Use `Arc<Mutex<Vec<usize>>>`.
pub fn concurrent_collect(n_threads: usize) -> Vec<usize> {
    let shared_vec = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    for thread_id in 0..n_threads {
        let vec_clone = Arc::clone(&shared_vec);
        let handle = thread::spawn(move || {
            let mut v = vec_clone.lock().unwrap();
            v.push(thread_id);
        });
        handles.push(handle);
    }

    // Join all threads
    for h in handles {
        h.join().unwrap();
    }

    // Take out vector and sort
    let mut result = shared_vec.lock().unwrap();
    result.sort();
    result.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_counter_single_thread() {
        assert_eq!(concurrent_counter(1, 100), 100);
    }
    #[test]
    fn test_counter_multi_thread() {
        assert_eq!(concurrent_counter(10, 100), 1000);
    }
    #[test]
    fn test_counter_zero() {
        assert_eq!(concurrent_counter(5, 0), 0);
    }
    #[test]
    fn test_collect() {
        let result = concurrent_collect(5);
        assert_eq!(result, vec![0, 1, 2, 3, 4]);
    }
    #[test]
    fn test_collect_single() {
        assert_eq!(concurrent_collect(1), vec![0]);
    }
}
