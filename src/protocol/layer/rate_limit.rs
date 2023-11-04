use std::{task::{Poll, Context}, io, pin::Pin};
use futures_core::Future;
use tokio::time::{Duration, Instant, Sleep};

#[derive(Debug)]
pub struct RateLimitLayer {
    /// 周期内可以通行的数据
    nums: u64,
    /// 每个周期的时间
    per: Duration,
    /// 当前周期下，还剩下可通行的数据
    left_nums: u64,
    /// 下一个时间重新计算的日期
    util: Instant,
    sleep: Pin<Box<Sleep>>,
}

impl RateLimitLayer {
    pub fn new(nums: u64, per: Duration) -> Self {
        let util = Instant::now();
        Self {
            nums,
            per,
            left_nums: nums,
            util,
            sleep: Box::pin(tokio::time::sleep_until(util)),
        }
    }

    pub fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.left_nums > 0 {
            return Poll::Ready(Ok(()));
        }

        if Pin::new(&mut self.sleep).poll(cx).is_pending() {
            tracing::trace!("rate limit exceeded; sleeping.");
            return Poll::Pending;
        }
        
        self.left_nums = self.nums;
        self.util = Instant::now() + self.per;
        self.sleep.as_mut().set(tokio::time::sleep_until(Instant::now() + self.per));
        return Poll::Ready(Ok(()));
    }

    pub fn poll_call(&mut self, mut count: u64) -> io::Result<()> {
        if self.left_nums == 0 {
            return Ok(());
        }

        let now = Instant::now();
        if now > self.util {
            self.nums = self.left_nums;
            self.util = now + self.per;
            // self.sleep.as_mut().set(tokio::time::sleep_until(self.util));
        }

        if self.left_nums > count {
            self.left_nums -= count;
            return Ok(())
        }

        count -= self.left_nums;

        let ratio = (count as f32 * 1.0f32) / (self.nums as f32) + 1.0f32;
        self.left_nums = 0;
        if self.left_nums == 0 {
            self.util += self.per.mul_f32(ratio);
            self.sleep.as_mut().set(tokio::time::sleep_until(self.util));
        }
        return Ok(());
    }
}
