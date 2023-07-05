use std::time::Instant;

use enum_ptr::Compact;

pub fn timed_scope<R, F: FnOnce() -> R>(label: &str, fun: F) -> R {
    let start = Instant::now();

    let res = fun();

    let time = Instant::now().duration_since(start);
    println!("{label} took: {time:?}");

    res
}

pub struct TaggedPtr<T>(pub Compact<T>)
where
    T: From<Compact<T>>,
    Compact<T>: From<T>;

impl<T> TaggedPtr<T>
where
    T: From<Compact<T>>,
    Compact<T>: From<T>,
{
    pub fn new(val: T) -> Self {
        Self(Compact::from(val))
    }
}
