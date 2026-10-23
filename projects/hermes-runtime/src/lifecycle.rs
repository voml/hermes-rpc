//! dispatch 生命周期：deadline、cancellation 与并发上限。
//!
//! 运行时是同步模型，因此 deadline 在 dispatch 前后各强制检查一次，handler 通过
//! [`CallContext`] 协作检查。取消用 [`CancelToken`]，调用方与 handler 都可以触发。
//! 超时与取消必须返回**可区分**的错误码（`deadline-exceeded` / `cancelled`），
//! 不能吞成通用失败。

use std::{
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crate::error::{RpcError, RpcErrorCode, RpcResult};

/// 等待并发许可时轮询取消与 deadline 的间隔。
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// 取消令牌。
///
/// 克隆出来的令牌共享同一取消状态：任一副本调用 [`CancelToken::cancel`] 后，
/// 所有副本都观察到已取消。
#[derive(Clone, Debug, Default)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    /// 新建一个未取消的令牌。
    pub fn new() -> Self {
        Self::default()
    }

    /// 标记取消。
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// 是否已取消。
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// 单次 dispatch 的生命周期参数。
///
/// 默认值表示不设 deadline、不绑定取消令牌。
#[derive(Clone, Debug, Default)]
pub struct CallOptions {
    deadline: Option<Instant>,
    cancellation: Option<CancelToken>,
}

impl CallOptions {
    /// 不设 deadline、不绑定取消令牌。
    pub fn new() -> Self {
        Self::default()
    }

    /// 以绝对时刻作为 deadline。
    pub fn with_deadline(deadline: Instant) -> Self {
        Self { deadline: Some(deadline), cancellation: None }
    }

    /// 以相对超时构造 deadline，截止时刻为 `Instant::now() + timeout`。
    pub fn with_timeout(timeout: Duration) -> Self {
        Self::with_deadline(Instant::now() + timeout)
    }

    /// 追加绝对 deadline。
    pub fn and_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// 追加相对超时。
    pub fn and_timeout(mut self, timeout: Duration) -> Self {
        self.deadline = Some(Instant::now() + timeout);
        self
    }

    /// 绑定取消令牌。
    pub fn and_cancellation(mut self, token: CancelToken) -> Self {
        self.cancellation = Some(token);
        self
    }

    /// 当前 deadline。
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// 当前取消令牌。
    pub fn cancellation(&self) -> Option<&CancelToken> {
        self.cancellation.as_ref()
    }
}

/// 传给 handler 的调用上下文。
///
/// handler 应周期性调用 [`CallContext::check`] 或在长循环里检查
/// [`CallContext::is_cancelled`] / [`CallContext::remaining`]，把超时与取消尽早
/// 转成 `deadline-exceeded` / `cancelled`。
#[derive(Clone, Debug)]
pub struct CallContext {
    options: CallOptions,
}

impl CallContext {
    pub(crate) fn new(options: CallOptions) -> Self {
        Self { options }
    }

    /// deadline，未设置时为 `None`。
    pub fn deadline(&self) -> Option<Instant> {
        self.options.deadline()
    }

    /// 距离 deadline 的剩余时间；未设 deadline 时为 `None`，已过期时为零值。
    pub fn remaining(&self) -> Option<Duration> {
        self.deadline().map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    /// 是否已被调用方或 handler 取消。
    pub fn is_cancelled(&self) -> bool {
        self.options.cancellation().is_some_and(CancelToken::is_cancelled)
    }

    /// 绑定的取消令牌。
    pub fn token(&self) -> Option<&CancelToken> {
        self.options.cancellation()
    }

    /// 统一检查取消与 deadline，命中时返回对应错误码。
    pub fn check(&self) -> RpcResult<()> {
        if self.is_cancelled() {
            return Err(cancelled());
        }
        if self.deadline().is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(deadline_exceeded());
        }
        Ok(())
    }
}

fn cancelled() -> RpcError {
    RpcError::new(RpcErrorCode::Cancelled, "dispatch cancelled")
}

fn deadline_exceeded() -> RpcError {
    RpcError::new(RpcErrorCode::DeadlineExceeded, "dispatch deadline exceeded")
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 在途调用上限。克隆共享同一份计数。
#[derive(Clone, Debug, Default)]
pub(crate) struct InFlightLimiter {
    state: Arc<InFlightState>,
}

#[derive(Debug, Default)]
struct InFlightState {
    inner: Mutex<InFlightInner>,
    available: Condvar,
}

#[derive(Debug, Default)]
struct InFlightInner {
    limit: Option<usize>,
    active: usize,
}

impl InFlightLimiter {
    /// 设置在途上限。`None` 表示不限。
    pub(crate) fn set_limit(&self, limit: Option<usize>) {
        lock(&self.state.inner).limit = limit;
        self.state.available.notify_all();
    }

    /// 当前在途上限。
    pub(crate) fn limit(&self) -> Option<usize> {
        lock(&self.state.inner).limit
    }

    /// 取得一个在途许可，必要时空闲等待。
    ///
    /// 等待期间持续检查取消与 deadline：命中任一条件立即返回对应错误码，不会
    /// 无限期占用调用线程。
    pub(crate) fn acquire(&self, ctx: &CallContext) -> RpcResult<InFlightGuard> {
        loop {
            ctx.check()?;

            let mut inner = lock(&self.state.inner);
            let at_capacity = inner.limit.is_some_and(|limit| inner.active >= limit);
            if !at_capacity {
                inner.active += 1;
                drop(inner);
                return Ok(InFlightGuard { state: Arc::clone(&self.state) });
            }

            let wait = match ctx.remaining() {
                Some(remaining) if remaining.is_zero() => return Err(deadline_exceeded()),
                Some(remaining) => remaining.min(POLL_INTERVAL),
                None => POLL_INTERVAL,
            };
            let (guard, _timeout) =
                self.state.available.wait_timeout(inner, wait).unwrap_or_else(|poisoned| poisoned.into_inner());
            drop(guard);
        }
    }
}

/// 在途许可。drop 时归还计数并唤醒一个等待者。
#[derive(Debug)]
pub(crate) struct InFlightGuard {
    state: Arc<InFlightState>,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        let mut inner = lock(&self.state.inner);
        inner.active = inner.active.saturating_sub(1);
        drop(inner);
        self.state.available.notify_one();
    }
}
