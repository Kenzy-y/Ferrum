//! Declaring token groups.

/// Declares a struct of tokens deserialized from a TOML table whose keys are
/// dotted names (`"bg.app"`). Every key is required and unknown keys are
/// rejected, so a theme file is always complete. Each field is documented
/// with its key; an explicit doc comment adds where the token is used.
macro_rules! token_struct {
    (
        $(#[$meta:meta])*
        $name:ident: $ty:ty {
            $( $(#[$field_meta:meta])* $field:ident = $key:literal, )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $(
                #[doc = concat!("`", $key, "`.")]
                $(#[$field_meta])*
                #[serde(rename = $key)]
                pub $field: $ty,
            )*
        }

        impl $name {
            /// Every token as `(key, value)`, in declaration order.
            pub fn entries(&self) -> Vec<(&'static str, $ty)> {
                vec![$(($key, self.$field)),*]
            }
        }
    };
}

pub(crate) use token_struct;
