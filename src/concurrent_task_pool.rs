//! concurrent_task_pool —— 一个基于 tokio 的并发任务池。
//!
//! 核心能力：
//! - `spawn`：提交一个异步任务，立即拿到 `TaskHandle`
//! - `cancel`：取消单个或全部任务（协作式信号，另有 abort 强制中止）
//! - `await_task`：等待任务结束并取回结果
//!
//! 只依赖 tokio，可作为独立模块通过 `mod concurrent_task_pool;` 引用，
//! 不需要整合进任何服务框架。

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{oneshot, watch};
use tokio::task::AbortHandle;

/// 任务的唯一标识。
pub type TaskId = u64;

/// 每个任务的登记信息（只放在池内部）。
struct TaskEntry {
    /// 取消信号发送端：send(true) 即请求取消。
    cancel: watch::Sender<bool>,
    /// 强制中止句柄：abort() 直接杀掉任务。
    abort: AbortHandle,
}

/// 池的内部状态，用 Arc 共享给每个任务，任务结束后自动注销自己。
struct Inner {
    tasks: Mutex<HashMap<TaskId, TaskEntry>>,
    next_id: AtomicU64,
}

/// 并发任务池。
pub struct TaskPool {
    inner: Arc<Inner>,
}

impl TaskPool {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                tasks: Mutex::new(HashMap::new()),
                next_id: AtomicU64::new(0),
            }),
        }
    }

    /// 提交一个异步任务，立即返回句柄，不阻塞。
    ///
    /// 任务被包在 `tokio::select!` 里：要么正常完成、要么响应取消信号。
    pub fn spawn<F, T>(&self, fut: F) -> TaskHandle<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let (result_tx, result_rx) = oneshot::channel();

        let worker = tokio::spawn(run_task(
            fut,
            cancel_rx,
            result_tx,
            self.inner.clone(),
            id,
        ));

        self.inner.tasks.lock().unwrap().insert(
            id,
            TaskEntry {
                cancel: cancel_tx.clone(),
                abort: worker.abort_handle(),
            },
        );

        TaskHandle {
            id,
            cancel: cancel_tx,
            abort: worker.abort_handle(),
            rx: result_rx,
            inner: self.inner.clone(),
        }
    }

    /// 协作式取消指定任务。返回是否找到了该任务。
    ///
    /// 发送取消信号后，任务会在下一次让出执行权时退出；
    /// 若任务内部不响应信号，请改用 [`TaskPool::abort`] 强制中止。
    pub fn cancel(&self, id: TaskId) -> bool {
        match self.inner.tasks.lock().unwrap().remove(&id) {
            Some(entry) => {
                let _ = entry.cancel.send(true);
                true
            }
            None => false,
        }
    }

    /// 强制中止指定任务（不等待任务清理）。
    pub fn abort(&self, id: TaskId) -> bool {
        match self.inner.tasks.lock().unwrap().remove(&id) {
            Some(entry) => {
                entry.abort.abort();
                true
            }
            None => false,
        }
    }

    /// 取消池中所有任务。
    pub fn cancel_all(&self) {
        for (_, entry) in self.inner.tasks.lock().unwrap().drain() {
            let _ = entry.cancel.send(true);
        }
    }

    /// 强制中止池中所有任务。
    pub fn abort_all(&self) {
        for (_, entry) in self.inner.tasks.lock().unwrap().drain() {
            entry.abort.abort();
        }
    }

    /// 当前仍在池中登记的任务数。
    pub fn len(&self) -> usize {
        self.inner.tasks.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for TaskPool {
    fn default() -> Self {
        Self::new()
    }
}

/// 任务的句柄：用于等待结果或取消任务。
pub struct TaskHandle<T> {
    id: TaskId,
    cancel: watch::Sender<bool>,
    abort: AbortHandle,
    rx: oneshot::Receiver<T>,
    inner: Arc<Inner>,
}

impl<T> TaskHandle<T> {
    pub fn id(&self) -> TaskId {
        self.id
    }

    /// 协作式取消该任务。
    pub fn cancel(&self) {
        let _ = self.cancel.send(true);
    }

    /// 强制中止该任务，并从池中注销。
    pub fn abort(&self) {
        self.inner.tasks.lock().unwrap().remove(&self.id);
        self.abort.abort();
    }

    /// 等待任务结果。任务被取消或未正常产出结果时返回 `None`。
    ///
    /// 取消时任务 future 会被内部 `select!` drop 掉，因此无需任务
    /// 内部主动配合即可停止。
    pub async fn await_task(self) -> Option<T> {
        self.rx.await.ok()
    }
}

/// 任务包装：在「取消」与「任务完成」之间竞速。
///
/// 无论走哪条分支，结束后都会从池中注销自己。
async fn run_task<F, T>(
    fut: F,
    mut cancel_rx: watch::Receiver<bool>,
    result_tx: oneshot::Sender<T>,
    inner: Arc<Inner>,
    id: TaskId,
) where
    F: Future<Output = T> + Send,
    T: Send,
{
    tokio::select! {
        _ = wait_cancelled(&mut cancel_rx) => {
            // 收到取消信号：丢弃 result_tx，不发送结果
        }
        result = fut => {
            let _ = result_tx.send(result);
        }
    }

    // 任务结束，从池中移除自己（幂等，重复移除无害）。
    inner.tasks.lock().unwrap().remove(&id);
}

/// 等待取消信号变为 true；发送端全部 drop 时也退出。
async fn wait_cancelled(rx: &mut watch::Receiver<bool>) {
    while !*rx.borrow() {
        if rx.changed().await.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn await_returns_result() {
        let pool = TaskPool::new();
        let handle = pool.spawn(async { 1 + 1 });
        assert_eq!(handle.await_task().await, Some(2));
        assert!(pool.is_empty());
    }

    #[tokio::test]
    async fn cancel_returns_none() {
        let pool = TaskPool::new();
        let handle = pool.spawn(async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            42
        });
        assert!(pool.cancel(handle.id()));
        assert_eq!(handle.await_task().await, None);
    }

    #[tokio::test]
    async fn many_tasks_run_concurrently() {
        let pool = TaskPool::new();
        let handles: Vec<_> = (0..10)
            .map(|i| pool.spawn(async move { i * 2 }))
            .collect();
        for (i, h) in handles.into_iter().enumerate() {
            assert_eq!(h.await_task().await, Some(i * 2));
        }
        assert!(pool.is_empty());
    }

    #[tokio::test]
    async fn cancel_all_clears_pool() {
        let pool = TaskPool::new();
        for i in 0..5 {
            pool.spawn(async move {
                tokio::time::sleep(Duration::from_secs(10)).await;
                i
            });
        }
        assert_eq!(pool.len(), 5);
        pool.cancel_all();
        assert!(pool.is_empty());
    }
}
