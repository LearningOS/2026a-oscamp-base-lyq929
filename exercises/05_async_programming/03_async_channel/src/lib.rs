//! # Async Channel
//!
//! In this exercise, you will use `tokio::sync::mpsc` async channels to implement producer-consumer pattern.
//!
//! ## Concepts
//! - `tokio::sync::mpsc::channel` creates bounded async channels
//! - Async `send` and `recv`
//! - Channel closing mechanism (receiver returns None after all senders are dropped)
use tokio::sync::mpsc;
/// Async producer-consumer:
/// - Create a producer task that sends each element from items sequentially
/// - Create a consumer task that receives all elements and collects them into Vec for return
///
/// Hint: Set channel capacity to items.len().max(1)
pub async fn producer_consumer(items: Vec<String>) -> Vec<String> {
    let cap = items.len().max(1);
    let (tx, mut rx) = mpsc::channel(cap);

    // producer task
    let producer = tokio::spawn(async move {
        for item in items {
            tx.send(item).await.unwrap();
        }
    });

    // consumer task
    let consumer = tokio::spawn(async move {
        let mut res = Vec::new();
        while let Some(msg) = rx.recv().await {
            res.push(msg);
        }
        res
    });

    // wait producer finish, then join consumer and return collected vector
    producer.await.unwrap();
    let result = consumer.await.unwrap();
    result
}

/// Fan‑in pattern: multiple producers, one consumer.
/// Create `n_producers` producers, each sending `"producer {id}: message"`.
/// Consumer collects all messages, sorts them, and returns.
pub async fn fan_in(n_producers: usize) -> Vec<String> {
    let (tx, mut rx) = mpsc::channel(10);
    let mut handles = Vec::new();

    for id in 0..n_producers {
        let tx_clone = tx.clone();
        let h = tokio::spawn(async move {
            let msg = format!("producer {id}: message");
            tx_clone.send(msg).await.unwrap();
        });
        handles.push(h);
    }

    // Drop original sender, only clones remain in producer tasks
    drop(tx);

    // consumer collect all messages
    let mut messages = Vec::new();
    while let Some(m) = rx.recv().await {
        messages.push(m);
    }

    // wait all producers done
    for h in handles {
        h.await.unwrap();
    }

    messages.sort();
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_producer_consumer() {
        let items = vec!["hello".into(), "async".into(), "world".into()];
        let result = producer_consumer(items.clone()).await;
        assert_eq!(result, items);
    }
    #[tokio::test]
    async fn test_producer_consumer_empty() {
        let result = producer_consumer(vec![]).await;
        assert!(result.is_empty());
    }
    #[tokio::test]
    async fn test_fan_in() {
        let result = fan_in(3).await;
        assert_eq!(
            result,
            vec![
                "producer 0: message",
                "producer 1: message",
                "producer 2: message",
            ]
        );
    }
    #[tokio::test]
    async fn test_fan_in_single() {
        let result = fan_in(1).await;
        assert_eq!(result, vec!["producer 0: message"]);
    }
}
