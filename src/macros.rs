// Macro for upgrading a weak reference or returning the given value
//
// This works for glib/gtk objects as well as anything else providing an upgrade method
macro_rules! upgrade_weak {
    ($x:ident, $r:expr) => {{
        match $x.upgrade() {
            Some(o) => o,
            None => return $r,
        }
    }};
    ($x:ident) => {
        upgrade_weak!($x, ())
    };
}

macro_rules! str_some_value (
    ($value:expr, $t:ty) => (
        {
            let value = $value.get::<$t>().expect("ser_some_value macro");
            value
        }
    );
);
macro_rules! str_opt_value (
    ($value:expr, $t:ty) => (
        {
            match $value.get::<$t>() {
                Ok(v) => Some(v),
                Err(_e) => None
            }
        }
    );
);
