//! concurrent_task_pool —— 一个基于 tokio 的并发任务池。
//!
//! 核心能力：
//! - `spawn`：提交一个异步任务，立即拿到 `TaskHandle`
//! - `cancel`：取消单个或全部任务（协作式信号，另有 abort 强制中止）
//! - `await_task`：等待任务结束并取回结果
//! - `status` / `result`：查询任务状态、读取已完成任务的结果
//!
//! 只依赖 tokio，不引入任何 HTTP 依赖，可作为独立模块被引用。

use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{oneshot, watch, OwnedSemaphorePermit, Semaphore};
use tokio::task::AbortHandle;

/// 任务的唯一标识。
pub type TaskId = u64;

/// 任务状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// 任务仍在运行。
    Running,
    /// 任务已完成，结果可通过 [`TaskPool::result`] 查询。
    Completed,
}

/// 任务运行时的共享状态：状态 + 可选结果。
struct TaskState {
    status: TaskStatus,
    result: Option<Arc<dyn Any + Send + Sync>>,
}

impl TaskState {
    fn new() -> Self {
        Self {
            status: TaskStatus::Running,
            result: None,
        }
    }
}

/// 每个任务的登记信息（只放在池内部）。
struct TaskEntry {
    /// 取消信号发送端：send(true) 即请求取消。
    cancel: watch::Sender<bool>,
    /// 强制中止句柄：abort() 直接杀掉任务。
    abort: AbortHandle,
    /// 共享状态，供池外部查询。
    state: Arc<Mutex<TaskState>>,
}

/// 池的内部状态，用 Arc 共享给每个任务。
struct Inner {
    tasks: Mutex<HashMap<TaskId, TaskEntry>>,
    next_id: AtomicU64,
}

/// 并发任务池。
pub struct TaskPool {
    inner: Arc<Inner>,
    /// 并发上限信号量：限制同时运行的任务数。
    semaphore: Arc<Semaphore>,
}

impl TaskPool {
    /// 默认最大并发数。
    pub const DEFAULT_MAX_CONCURRENT: usize = 64;

    pub fn new() -> Self {
        Self::with_max_concurrent(Self::DEFAULT_MAX_CONCURRENT)
    }

    /// 以指定的最大并发数创建任务池。
    pub fn with_max_concurrent(max: usize) -> Self {
        assert!(max > 0, "最大并发数必须大于 0");
        Self {
            inner: Arc::new(Inner {
                tasks: Mutex::new(HashMap::new()),
                next_id: AtomicU64::new(0),
            }),
            semaphore: Arc::new(Semaphore::new(max)),
        }
    }

    /// 提交一个异步任务，立即返回句柄，不阻塞。
    ///
    /// 任务被包在 `tokio::select!` 里：要么正常完成、要么响应取消信号。
    /// 任务完成后会保留在池中（状态为 [`TaskStatus::Completed`]），
    /// 以便通过 [`TaskPool::status`] / [`TaskPool::result`] 查询。
    pub async fn spawn<F, T>(&self, fut: F) -> TaskHandle<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + Sync + 'static,
    {
        // 先获取许可：并发已满时在此等待，直到有任务结束释放许可。
        // permit 随任务 move 进 run_task，任务结束时自动 drop 释放。
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("Semaphore 永不关闭，acquire 不会失败");

        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let (done_tx, done_rx) = oneshot::channel();
        let state = Arc::new(Mutex::new(TaskState::new()));

        let worker = tokio::spawn(run_task(
            fut,
            cancel_rx,
            done_tx,
            state.clone(),
            self.inner.clone(),
            id,
            permit,
        ));

        self.inner.tasks.lock().unwrap().insert(
            id,
            TaskEntry {
                cancel: cancel_tx.clone(),
                abort: worker.abort_handle(),
                state: state.clone(),
            },
        );

        TaskHandle {
            id,
            cancel: cancel_tx,
            abort: worker.abort_handle(),
            rx: done_rx,
            state,
            inner: self.inner.clone(),
            _marker: PhantomData,
        }
    }

    /// 协作式取消指定任务。返回是否找到了该任务。
    ///
    /// 取消后任务从池中移除，查询该 id 会得到 `None`（not_found）。
    pub fn cancel(&self, id: TaskId) -> bool {
        match self.inner.tasks.lock().unwrap().remove(&id) {
            Some(entry) => {
                let _ = entry.cancel.send(true);
                true
            }
            None => false,
        }
    }

    /// 强制中止指定任务，并从池中移除。
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

    /// 查询任务状态；任务不存在返回 `None`。
    pub fn status(&self, id: TaskId) -> Option<TaskStatus> {
        let tasks = self.inner.tasks.lock().unwrap();
        tasks.get(&id).map(|e| e.state.lock().unwrap().status)
    }

    /// 读取已完成任务的结果（不消费，可重复查询）。
    ///
    /// 类型不匹配或任务未完成时返回 `None`。
    pub fn result<T: Send + Sync + 'static>(&self, id: TaskId) -> Option<Arc<T>> {
        let tasks = self.inner.tasks.lock().unwrap();
        let entry = tasks.get(&id)?;
        let state = entry.state.lock().unwrap();
        let arc = state.result.as_ref()?.clone();
        arc.downcast::<T>().ok()
    }

    /// 当前登记在池中的任务数（含已完成待查询的任务）。
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
    /// 完成通知：任务结束后收到一个空信号。
    rx: oneshot::Receiver<()>,
    /// 共享状态，`await_task` 从这里取结果。
    state: Arc<Mutex<TaskState>>,
    inner: Arc<Inner>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Send + Sync + 'static> TaskHandle<T> {
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
    /// 注意：本方法会取走结果（一次性消费）；如需重复查询，
    /// 请改用 [`TaskPool::result`]。
    pub async fn await_task(self) -> Option<T> {
        self.rx.await.ok()?;
        let mut state = self.state.lock().unwrap();
        let arc = state.result.take()?.downcast::<T>().ok()?;
        Arc::try_unwrap(arc).ok()
    }
}

/// 任务包装：在「取消」与「任务完成」之间竞速。
///
/// 完成分支会把结果写入共享状态并保留在池中；
/// 取消分支会把任务从池中移除。
async fn run_task<F, T>(
    fut: F,
    mut cancel_rx: watch::Receiver<bool>,
    done_tx: oneshot::Sender<()>,
    state: Arc<Mutex<TaskState>>,
    inner: Arc<Inner>,
    id: TaskId,
    // 持有并发许可直到本函数返回；drop 时自动释放名额
    _permit: OwnedSemaphorePermit,
) where
    F: Future<Output = T> + Send,
    T: Send + Sync + 'static,
{
    tokio::select! {
        _ = wait_cancelled(&mut cancel_rx) => {
            inner.tasks.lock().unwrap().remove(&id);
        }
        result = fut => {
            {
                let mut s = state.lock().unwrap();
                s.status = TaskStatus::Completed;
                s.result = Some(Arc::new(result));
            }
            let _ = done_tx.send(());
        }
    }
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
        let handle = pool.spawn(async { 1 + 1 }).await;
        let id = handle.id();
        assert_eq!(handle.await_task().await, Some(2));
        assert_eq!(pool.status(id), Some(TaskStatus::Completed));
    }

    #[tokio::test]
    async fn status_and_result_after_completion() {
        let pool = TaskPool::new();
        let handle = pool.spawn(async { 42 }).await;
        assert_eq!(pool.status(handle.id()), Some(TaskStatus::Running));
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(pool.status(handle.id()), Some(TaskStatus::Completed));
        assert_eq!(pool.result::<i32>(handle.id()).map(|r| *r), Some(42));
    }

    #[tokio::test]
    async fn result_can_be_queried_multiple_times() {
        let pool = TaskPool::new();
        let handle = pool.spawn(async { 7 }).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(pool.result::<i32>(handle.id()).map(|r| *r), Some(7));
        assert_eq!(pool.result::<i32>(handle.id()).map(|r| *r), Some(7));
    }

    #[tokio::test]
    async fn cancel_returns_none_and_removes_task() {
        let pool = TaskPool::new();
        let handle = pool
            .spawn(async {
                tokio::time::sleep(Duration::from_secs(10)).await;
                42
            })
            .await;
        let id = handle.id();
        assert!(pool.cancel(id));
        assert_eq!(handle.await_task().await, None);
        assert_eq!(pool.status(id), None);
    }

    #[tokio::test]
    async fn many_tasks_run_concurrently() {
        let pool = TaskPool::new();
        let mut handles = Vec::new();
        for i in 0..10 {
            handles.push(pool.spawn(async move { i * 2 }).await);
        }
        for (i, h) in handles.into_iter().enumerate() {
            assert_eq!(h.await_task().await, Some(i * 2));
        }
    }

    #[tokio::test]
    async fn cancel_all_clears_pool() {
        let pool = TaskPool::new();
        for i in 0..5 {
            pool.spawn(async move {
                tokio::time::sleep(Duration::from_secs(10)).await;
                i
            })
            .await;
        }
        assert_eq!(pool.len(), 5);
        pool.cancel_all();
        assert!(pool.is_empty());
    }

    #[tokio::test]
    async fn concurrent_limit_is_enforced() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let pool = Arc::new(TaskPool::with_max_concurrent(2));
        let running = Arc::new(AtomicUsize::new(0));
        let max_observed = Arc::new(AtomicUsize::new(0));

        // 并发提交 5 个任务，每个任务 sleep 50ms
        let mut submissions = Vec::new();
        for i in 0..5 {
            let pool = pool.clone();
            let running = running.clone();
            let max_observed = max_observed.clone();
            submissions.push(tokio::spawn(async move {
                pool.spawn(async move {
                    let cur = running.fetch_add(1, Ordering::SeqCst) + 1;
                    max_observed.fetch_max(cur, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    running.fetch_sub(1, Ordering::SeqCst);
                    i
                })
                .await
            }));
        }

        for s in submissions {
            s.await.unwrap();
        }

        let peak = max_observed.load(Ordering::SeqCst);
        assert!(peak >= 2, "应观察到至少 2 个任务并发，实际峰值 {peak}");
        assert!(peak <= 2, "并发数不应超过上限 2，实际峰值 {peak}");
    }
}
