#[macro_export]
macro_rules! convert_enum{($src: ident, $dst: ident, $($variant: ident,)*)=> {
    impl From<$src> for $dst {
        fn from(src: $src) -> Self {
            match src {
                $($src::$variant => Self::$variant,)*
            }
        }
    }
}}

#[macro_export]
macro_rules! convert_struct{($src: ident, $dst: ident, {$($field: ident),*})=> {
	impl From<$src> for $dst {
		fn from(src: $src) -> Self {
			Self {
				$($field: src.$field.into(),)*
			}
		}
	}
}}
