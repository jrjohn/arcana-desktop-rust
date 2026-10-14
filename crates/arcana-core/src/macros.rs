//! Helper macros shared by the domain and data layers.

/// Declare a C#-style integer-backed enum: the enum itself plus `ALL`, `value()` and
/// `from_value()` so it can be persisted as an integer exactly like EF Core does.
#[macro_export]
macro_rules! int_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($(#[$vmeta:meta])* $variant:ident = $value:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $($(#[$vmeta])* $variant = $value),+
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The integer value (the C# enum value).
            #[must_use]
            pub fn value(self) -> i32 {
                self as i32
            }

            /// Reverse of `value()`; `None` for an unknown integer.
            #[must_use]
            pub fn from_value(value: i32) -> Option<Self> {
                match value {
                    $($value => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    crate::int_enum! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Sample { A = 0, B = 5 }
    }

    #[test]
    fn int_enum_round_trips() {
        assert_eq!(Sample::ALL, &[Sample::A, Sample::B]);
        assert_eq!(Sample::B.value(), 5);
        assert_eq!(Sample::from_value(0), Some(Sample::A));
        assert_eq!(Sample::from_value(1), None);
    }
}
