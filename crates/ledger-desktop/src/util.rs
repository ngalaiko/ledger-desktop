use gpui::{Context, Entity, Subscription};

/// Trait for observing multiple entities at once.
pub trait ObserveMultiple<T: 'static> {
    fn observe_all(
        &self,
        cx: &mut Context<T>,
        callback: impl Fn(&mut T, &mut Context<T>) + Clone + 'static,
    ) -> Subscription;
}

/// Observe multiple entities and run callback when any of them changes.
///
/// # Example
/// ```ignore
/// let subscription = observe_multiple(
///     cx,
///     (&ledger_file, &app_state),
///     |this, cx| {
///         this.recalculate(cx);
///         cx.notify();
///     },
/// );
/// ```
pub fn observe_multiple<T: 'static, M: ObserveMultiple<T>>(
    cx: &mut Context<T>,
    entities: M,
    callback: impl Fn(&mut T, &mut Context<T>) + Clone + 'static,
) -> Subscription {
    entities.observe_all(cx, callback)
}

impl<T: 'static, E1: 'static, E2: 'static> ObserveMultiple<T> for (&Entity<E1>, &Entity<E2>) {
    fn observe_all(
        &self,
        cx: &mut Context<T>,
        callback: impl Fn(&mut T, &mut Context<T>) + Clone + 'static,
    ) -> Subscription {
        let s1 = cx.observe(self.0, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        let s2 = cx.observe(self.1, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        Subscription::join(s1, s2)
    }
}

impl<T: 'static, E1: 'static, E2: 'static, E3: 'static> ObserveMultiple<T>
    for (&Entity<E1>, &Entity<E2>, &Entity<E3>)
{
    fn observe_all(
        &self,
        cx: &mut Context<T>,
        callback: impl Fn(&mut T, &mut Context<T>) + Clone + 'static,
    ) -> Subscription {
        let s1 = cx.observe(self.0, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        let s2 = cx.observe(self.1, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        let s3 = cx.observe(self.2, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        Subscription::join(Subscription::join(s1, s2), s3)
    }
}

impl<T: 'static, E1: 'static, E2: 'static, E3: 'static, E4: 'static> ObserveMultiple<T>
    for (&Entity<E1>, &Entity<E2>, &Entity<E3>, &Entity<E4>)
{
    fn observe_all(
        &self,
        cx: &mut Context<T>,
        callback: impl Fn(&mut T, &mut Context<T>) + Clone + 'static,
    ) -> Subscription {
        let s1 = cx.observe(self.0, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        let s2 = cx.observe(self.1, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        let s3 = cx.observe(self.2, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        let s4 = cx.observe(self.3, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        Subscription::join(Subscription::join(Subscription::join(s1, s2), s3), s4)
    }
}

impl<T: 'static, E1: 'static, E2: 'static, E3: 'static, E4: 'static, E5: 'static> ObserveMultiple<T>
    for (
        &Entity<E1>,
        &Entity<E2>,
        &Entity<E3>,
        &Entity<E4>,
        &Entity<E5>,
    )
{
    fn observe_all(
        &self,
        cx: &mut Context<T>,
        callback: impl Fn(&mut T, &mut Context<T>) + Clone + 'static,
    ) -> Subscription {
        let s1 = cx.observe(self.0, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        let s2 = cx.observe(self.1, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        let s3 = cx.observe(self.2, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        let s4 = cx.observe(self.3, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        let s5 = cx.observe(self.4, {
            let cb = callback.clone();
            move |this, _, cx| cb(this, cx)
        });
        Subscription::join(
            Subscription::join(Subscription::join(Subscription::join(s1, s2), s3), s4),
            s5,
        )
    }
}
