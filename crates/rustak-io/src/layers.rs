use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use futures::future::BoxFuture;

use crate::{IoError, MessageEnvelope, MessageSink};

pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

pub struct TapLayer<S, F> {
    inner: S,
    on_envelope: F,
}

impl<S, F> TapLayer<S, F> {
    #[must_use]
    pub fn new(inner: S, on_envelope: F) -> Self {
        Self { inner, on_envelope }
    }

    #[must_use]
    pub fn inner(&self) -> &S {
        &self.inner
    }
}

impl<S, F, T> MessageSink<T> for TapLayer<S, F>
where
    S: MessageSink<T>,
    F: Fn(&MessageEnvelope<T>) + Send + Sync,
{
    fn send(&self, msg: T) -> BoxFuture<'_, Result<(), IoError>> {
        self.inner.send(msg)
    }

    fn send_envelope(&self, env: MessageEnvelope<T>) -> BoxFuture<'_, Result<(), IoError>> {
        (self.on_envelope)(&env);
        self.inner.send_envelope(env)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitConfig {
    pub max_events: usize,
    pub per: Duration,
}

impl RateLimitConfig {
    pub fn validate(&self) -> Result<(), IoError> {
        if self.max_events == 0 {
            return Err(IoError::Other(
                "rate-limit max_events must be greater than zero".to_string(),
            ));
        }
        if self.per.is_zero() {
            return Err(IoError::Other(
                "rate-limit period must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }
}

pub struct RateLimitLayer<S, C = SystemClock> {
    inner: S,
    config: RateLimitConfig,
    clock: C,
    window: Mutex<VecDeque<Instant>>,
}

impl<S> RateLimitLayer<S, SystemClock> {
    pub fn new(inner: S, config: RateLimitConfig) -> Result<Self, IoError> {
        config.validate()?;
        Ok(Self {
            inner,
            config,
            clock: SystemClock,
            window: Mutex::new(VecDeque::new()),
        })
    }
}

impl<S, C> RateLimitLayer<S, C>
where
    C: Clock,
{
    pub fn with_clock(inner: S, config: RateLimitConfig, clock: C) -> Result<Self, IoError> {
        config.validate()?;
        Ok(Self {
            inner,
            config,
            clock,
            window: Mutex::new(VecDeque::new()),
        })
    }

    fn acquire_slot(&self) -> bool {
        let now = self.clock.now();
        let mut window = self.window.lock().expect("rate-limit mutex poisoned");
        while let Some(oldest) = window.front() {
            if now.duration_since(*oldest) >= self.config.per {
                window.pop_front();
            } else {
                break;
            }
        }

        if window.len() >= self.config.max_events {
            return false;
        }

        window.push_back(now);
        true
    }
}

impl<S, C, T> MessageSink<T> for RateLimitLayer<S, C>
where
    S: MessageSink<T>,
    C: Clock,
{
    fn send(&self, msg: T) -> BoxFuture<'_, Result<(), IoError>> {
        if !self.acquire_slot() {
            return Box::pin(async { Err(IoError::Overloaded) });
        }
        self.inner.send(msg)
    }

    fn send_envelope(&self, env: MessageEnvelope<T>) -> BoxFuture<'_, Result<(), IoError>> {
        if !self.acquire_slot() {
            return Box::pin(async { Err(IoError::Overloaded) });
        }
        self.inner.send_envelope(env)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DedupConfig {
    pub max_keys: usize,
}

impl DedupConfig {
    pub fn validate(&self) -> Result<(), IoError> {
        if self.max_keys == 0 {
            return Err(IoError::Other(
                "dedup max_keys must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }
}

struct DedupState<K> {
    seen: HashSet<K>,
    order: VecDeque<K>,
}

pub struct DedupLayer<S, K, F> {
    inner: S,
    config: DedupConfig,
    key_fn: F,
    state: Mutex<DedupState<K>>,
}

impl<S, K, F> DedupLayer<S, K, F>
where
    K: Eq + Hash + Clone,
    F: Send + Sync,
{
    pub fn new(inner: S, config: DedupConfig, key_fn: F) -> Result<Self, IoError> {
        config.validate()?;
        Ok(Self {
            inner,
            config,
            key_fn,
            state: Mutex::new(DedupState {
                seen: HashSet::new(),
                order: VecDeque::new(),
            }),
        })
    }

    fn should_forward(&self, key: K) -> bool {
        let mut state = self.state.lock().expect("dedup mutex poisoned");
        if state.seen.contains(&key) {
            return false;
        }

        state.seen.insert(key.clone());
        state.order.push_back(key);

        while state.order.len() > self.config.max_keys {
            if let Some(evicted) = state.order.pop_front() {
                state.seen.remove(&evicted);
            }
        }

        true
    }
}

impl<S, K, F, T> MessageSink<T> for DedupLayer<S, K, F>
where
    S: MessageSink<T>,
    K: Eq + Hash + Clone + Send + Sync + 'static,
    F: Fn(&MessageEnvelope<T>) -> K + Send + Sync,
{
    fn send(&self, msg: T) -> BoxFuture<'_, Result<(), IoError>> {
        self.inner.send(msg)
    }

    fn send_envelope(&self, env: MessageEnvelope<T>) -> BoxFuture<'_, Result<(), IoError>> {
        let key = (self.key_fn)(&env);
        if !self.should_forward(key) {
            return Box::pin(async { Ok(()) });
        }
        self.inner.send_envelope(env)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoalesceConfig {
    pub max_keys: usize,
}

impl CoalesceConfig {
    pub fn validate(&self) -> Result<(), IoError> {
        if self.max_keys == 0 {
            return Err(IoError::Other(
                "coalesce max_keys must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoalesceAction {
    Inserted,
    Replaced,
}

struct CoalesceState<K, T> {
    entries: HashMap<K, MessageEnvelope<T>>,
    order: VecDeque<K>,
}

pub struct CoalesceLatestLayer<K, T, F> {
    config: CoalesceConfig,
    key_fn: F,
    state: Mutex<CoalesceState<K, T>>,
}

impl<K, T, F> CoalesceLatestLayer<K, T, F>
where
    K: Eq + Hash + Clone,
    F: Fn(&MessageEnvelope<T>) -> K + Send + Sync,
{
    pub fn new(config: CoalesceConfig, key_fn: F) -> Result<Self, IoError> {
        config.validate()?;
        Ok(Self {
            config,
            key_fn,
            state: Mutex::new(CoalesceState {
                entries: HashMap::new(),
                order: VecDeque::new(),
            }),
        })
    }

    pub fn enqueue(&self, env: MessageEnvelope<T>) -> CoalesceAction {
        let key = (self.key_fn)(&env);
        let mut state = self.state.lock().expect("coalesce mutex poisoned");

        let replaced = state.entries.insert(key.clone(), env).is_some();
        if !replaced {
            state.order.push_back(key.clone());
        }

        while state.entries.len() > self.config.max_keys {
            if let Some(oldest) = state.order.pop_front() {
                state.entries.remove(&oldest);
            }
        }

        if replaced {
            CoalesceAction::Replaced
        } else {
            CoalesceAction::Inserted
        }
    }

    pub async fn drain_into<S>(&self, sink: &S) -> Result<usize, IoError>
    where
        K: Ord,
        S: MessageSink<T>,
    {
        let mut state = self.state.lock().expect("coalesce mutex poisoned");
        let mut items: Vec<(K, MessageEnvelope<T>)> = state.entries.drain().collect();
        state.order.clear();
        drop(state);

        items.sort_by(|(left, _), (right, _)| left.cmp(right));
        let total = items.len();
        for (_, envelope) in items {
            sink.send_envelope(envelope).await?;
        }

        Ok(total)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImpairmentConfig {
    pub loss_probability: f64,
    pub duplicate_probability: f64,
    pub min_latency: Duration,
    pub max_latency: Duration,
    pub reorder_probability: f64,
}

impl ImpairmentConfig {
    pub fn validate(&self) -> Result<(), IoError> {
        for (name, value) in [
            ("loss_probability", self.loss_probability),
            ("duplicate_probability", self.duplicate_probability),
            ("reorder_probability", self.reorder_probability),
        ] {
            if !(0.0..=1.0).contains(&value) {
                return Err(IoError::Other(format!("{name} must be within [0.0, 1.0]")));
            }
        }
        if self.min_latency > self.max_latency {
            return Err(IoError::Other(
                "min_latency must be less than or equal to max_latency".to_string(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ImpairmentOutcome<T> {
    Drop,
    Forward {
        envelope: MessageEnvelope<T>,
        delay: Duration,
        reordered: bool,
    },
    Duplicate {
        first: MessageEnvelope<T>,
        second: MessageEnvelope<T>,
        delay: Duration,
        reordered: bool,
    },
}

pub struct ImpairmentLayer {
    config: ImpairmentConfig,
    rng_state: Mutex<u64>,
}

impl ImpairmentLayer {
    pub fn new(config: ImpairmentConfig, seed: u64) -> Result<Self, IoError> {
        config.validate()?;
        Ok(Self {
            config,
            rng_state: Mutex::new(if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            }),
        })
    }

    #[must_use]
    pub fn classify<T>(&self, envelope: MessageEnvelope<T>) -> ImpairmentOutcome<T>
    where
        T: Clone,
    {
        let mut state = self.rng_state.lock().expect("impairment mutex poisoned");
        let loss_roll = random_unit(state_ref(&mut state));
        if loss_roll < self.config.loss_probability {
            return ImpairmentOutcome::Drop;
        }

        let duplicate_roll = random_unit(state_ref(&mut state));
        let reorder_roll = random_unit(state_ref(&mut state));
        let latency_roll = random_unit(state_ref(&mut state));
        drop(state);

        let delay = jitter(
            self.config.min_latency,
            self.config.max_latency,
            latency_roll,
        );
        let reordered = reorder_roll < self.config.reorder_probability;
        if duplicate_roll < self.config.duplicate_probability {
            return ImpairmentOutcome::Duplicate {
                first: envelope.clone(),
                second: envelope,
                delay,
                reordered,
            };
        }

        ImpairmentOutcome::Forward {
            envelope,
            delay,
            reordered,
        }
    }
}

fn state_ref(value: &mut u64) -> &mut u64 {
    value
}

fn random_unit(state: &mut u64) -> f64 {
    let next = xorshift64(state);
    (next as f64) / (u64::MAX as f64)
}

fn xorshift64(state: &mut u64) -> u64 {
    let mut value = *state;
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    *state = value;
    value
}

fn jitter(min: Duration, max: Duration, roll: f64) -> Duration {
    if min == max {
        return min;
    }

    let span_nanos = max.saturating_sub(min).as_nanos();
    let offset_nanos = ((span_nanos as f64) * roll) as u128;
    let offset_u64 = u64::try_from(offset_nanos).unwrap_or(u64::MAX);
    min.checked_add(Duration::from_nanos(offset_u64))
        .unwrap_or(Duration::MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MetricsSnapshot {
    pub attempted: u64,
    pub sent: u64,
    pub dropped: u64,
    pub errors: u64,
}

pub struct MetricsLayer<S> {
    inner: S,
    attempted: AtomicU64,
    sent: AtomicU64,
    dropped: AtomicU64,
    errors: AtomicU64,
}

impl<S> MetricsLayer<S> {
    #[must_use]
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            attempted: AtomicU64::new(0),
            sent: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }

    pub fn record_drop(&self) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            attempted: self.attempted.load(Ordering::Relaxed),
            sent: self.sent.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
        }
    }
}

impl<S, T> MessageSink<T> for MetricsLayer<S>
where
    S: MessageSink<T>,
{
    fn send(&self, msg: T) -> BoxFuture<'_, Result<(), IoError>> {
        self.attempted.fetch_add(1, Ordering::Relaxed);
        let future = self.inner.send(msg);
        Box::pin(async move {
            let result = future.await;
            match &result {
                Ok(()) => {
                    self.sent.fetch_add(1, Ordering::Relaxed);
                }
                Err(_) => {
                    self.errors.fetch_add(1, Ordering::Relaxed);
                }
            }
            result
        })
    }

    fn send_envelope(&self, env: MessageEnvelope<T>) -> BoxFuture<'_, Result<(), IoError>> {
        self.attempted.fetch_add(1, Ordering::Relaxed);
        let future = self.inner.send_envelope(env);
        Box::pin(async move {
            let result = future.await;
            match &result {
                Ok(()) => {
                    self.sent.fetch_add(1, Ordering::Relaxed);
                }
                Err(_) => {
                    self.errors.fetch_add(1, Ordering::Relaxed);
                }
            }
            result
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant, SystemTime};

    use futures::executor::block_on;

    use super::{
        Clock, CoalesceAction, CoalesceConfig, CoalesceLatestLayer, DedupConfig, DedupLayer,
        ImpairmentConfig, ImpairmentLayer, ImpairmentOutcome, MetricsLayer, RateLimitConfig,
        RateLimitLayer, TapLayer,
    };
    use crate::{IoError, MessageEnvelope, MessageSink, ObservedTime};

    #[derive(Default)]
    struct CollectSink<T> {
        sent: Mutex<Vec<MessageEnvelope<T>>>,
        fail: bool,
    }

    impl<T> CollectSink<T> {
        fn failing() -> Self {
            Self {
                sent: Mutex::new(Vec::new()),
                fail: true,
            }
        }
    }

    impl<T: Send + 'static> MessageSink<T> for CollectSink<T> {
        fn send(&self, msg: T) -> futures::future::BoxFuture<'_, Result<(), IoError>> {
            Box::pin(async move {
                self.sent
                    .lock()
                    .expect("collect mutex poisoned")
                    .push(MessageEnvelope::new(msg));
                if self.fail {
                    Err(IoError::Other("forced failure".to_string()))
                } else {
                    Ok(())
                }
            })
        }

        fn send_envelope(
            &self,
            env: MessageEnvelope<T>,
        ) -> futures::future::BoxFuture<'_, Result<(), IoError>> {
            Box::pin(async move {
                self.sent.lock().expect("collect mutex poisoned").push(env);
                if self.fail {
                    Err(IoError::Other("forced failure".to_string()))
                } else {
                    Ok(())
                }
            })
        }
    }

    #[derive(Clone)]
    struct TestClock {
        now: Arc<Mutex<Instant>>,
    }

    impl TestClock {
        fn new(now: Instant) -> Self {
            Self {
                now: Arc::new(Mutex::new(now)),
            }
        }

        fn advance(&self, duration: Duration) {
            let mut now = self.now.lock().expect("clock mutex poisoned");
            *now = *now + duration;
        }
    }

    impl Clock for TestClock {
        fn now(&self) -> Instant {
            *self.now.lock().expect("clock mutex poisoned")
        }
    }

    #[test]
    fn tap_layer_observes_envelopes() {
        let sink = CollectSink::<String>::default();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let observed_clone = Arc::clone(&observed);
        let layer = TapLayer::new(sink, move |env: &MessageEnvelope<String>| {
            observed_clone
                .lock()
                .expect("observe mutex poisoned")
                .push(env.message.clone());
        });

        let env = MessageEnvelope::new("alpha".to_string());
        block_on(layer.send_envelope(env)).expect("tap send should succeed");

        let data = observed.lock().expect("observe mutex poisoned");
        assert_eq!(data.as_slice(), &["alpha".to_string()]);
    }

    #[test]
    fn rate_limit_layer_enforces_window_deterministically() {
        let sink = CollectSink::<String>::default();
        let clock = TestClock::new(Instant::now());
        let layer = RateLimitLayer::with_clock(
            sink,
            RateLimitConfig {
                max_events: 2,
                per: Duration::from_secs(1),
            },
            clock.clone(),
        )
        .expect("rate limit config should be valid");

        block_on(layer.send_envelope(MessageEnvelope::new("a".to_string())))
            .expect("first event should pass");
        block_on(layer.send_envelope(MessageEnvelope::new("b".to_string())))
            .expect("second event should pass");
        let third = block_on(layer.send_envelope(MessageEnvelope::new("c".to_string())));
        assert!(matches!(third, Err(IoError::Overloaded)));

        clock.advance(Duration::from_secs(1));
        block_on(layer.send_envelope(MessageEnvelope::new("d".to_string())))
            .expect("window should refill after period");
    }

    #[test]
    fn dedup_layer_drops_duplicate_keys() {
        let sink = CollectSink::<String>::default();
        let layer = DedupLayer::new(
            sink,
            DedupConfig { max_keys: 8 },
            |env: &MessageEnvelope<String>| env.message.clone(),
        )
        .expect("dedup config should be valid");

        block_on(layer.send_envelope(MessageEnvelope::new("alpha".to_string())))
            .expect("first key should pass");
        block_on(layer.send_envelope(MessageEnvelope::new("alpha".to_string())))
            .expect("duplicate should be dropped as success");
        block_on(layer.send_envelope(MessageEnvelope::new("beta".to_string())))
            .expect("new key should pass");

        let sent = layer.inner.sent.lock().expect("collect mutex poisoned");
        let messages: Vec<String> = sent.iter().map(|env| env.message.clone()).collect();
        assert_eq!(messages, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn coalesce_layer_drains_latest_per_key_in_stable_order() {
        let layer = CoalesceLatestLayer::new(
            CoalesceConfig { max_keys: 8 },
            |env: &MessageEnvelope<String>| env.message.chars().next().expect("non-empty message"),
        )
        .expect("coalesce config should be valid");

        assert_eq!(
            layer.enqueue(MessageEnvelope::new("a-1".to_string())),
            CoalesceAction::Inserted
        );
        assert_eq!(
            layer.enqueue(MessageEnvelope::new("b-1".to_string())),
            CoalesceAction::Inserted
        );
        assert_eq!(
            layer.enqueue(MessageEnvelope::new("a-2".to_string())),
            CoalesceAction::Replaced
        );

        let sink = CollectSink::<String>::default();
        let drained = block_on(layer.drain_into(&sink)).expect("drain should succeed");
        assert_eq!(drained, 2);

        let sent = sink.sent.lock().expect("collect mutex poisoned");
        let messages: Vec<String> = sent.iter().map(|env| env.message.clone()).collect();
        assert_eq!(messages, vec!["a-2".to_string(), "b-1".to_string()]);
    }

    #[test]
    fn impairment_layer_is_seed_deterministic() {
        let config = ImpairmentConfig {
            loss_probability: 0.1,
            duplicate_probability: 0.4,
            min_latency: Duration::from_millis(1),
            max_latency: Duration::from_millis(5),
            reorder_probability: 0.3,
        };

        let left = ImpairmentLayer::new(config, 42).expect("config should be valid");
        let right = ImpairmentLayer::new(config, 42).expect("config should be valid");

        let observed = ObservedTime::new(SystemTime::UNIX_EPOCH, Instant::now());
        let envelope = MessageEnvelope::new("packet".to_string()).with_observed(observed);

        for _ in 0..8 {
            let left_outcome = left.classify(envelope.clone());
            let right_outcome = right.classify(envelope.clone());
            assert_eq!(
                outcome_signature(&left_outcome),
                outcome_signature(&right_outcome)
            );
        }
    }

    #[test]
    fn metrics_layer_counts_attempts_successes_and_errors() {
        let success_sink = CollectSink::<String>::default();
        let success_layer = MetricsLayer::new(success_sink);
        block_on(success_layer.send_envelope(MessageEnvelope::new("ok".to_string())))
            .expect("send should succeed");
        success_layer.record_drop();

        let success_metrics = success_layer.snapshot();
        assert_eq!(success_metrics.attempted, 1);
        assert_eq!(success_metrics.sent, 1);
        assert_eq!(success_metrics.dropped, 1);
        assert_eq!(success_metrics.errors, 0);

        let failing_sink = CollectSink::<String>::failing();
        let failing_layer = MetricsLayer::new(failing_sink);
        let result =
            block_on(failing_layer.send_envelope(MessageEnvelope::new("fail".to_string())));
        assert!(matches!(result, Err(IoError::Other(_))));

        let failing_metrics = failing_layer.snapshot();
        assert_eq!(failing_metrics.attempted, 1);
        assert_eq!(failing_metrics.sent, 0);
        assert_eq!(failing_metrics.errors, 1);
    }

    fn outcome_signature(outcome: &ImpairmentOutcome<String>) -> (u8, Duration, bool) {
        match outcome {
            ImpairmentOutcome::Drop => (0, Duration::ZERO, false),
            ImpairmentOutcome::Forward {
                delay, reordered, ..
            } => (1, *delay, *reordered),
            ImpairmentOutcome::Duplicate {
                delay, reordered, ..
            } => (2, *delay, *reordered),
        }
    }
}
