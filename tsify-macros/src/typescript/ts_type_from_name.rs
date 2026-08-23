use crate::attrs::TypeGenerationConfig;

use super::{NullType, TsType, TsTypeElement, TsTypeLit};

/// Create a type literal with the given key-value pairs.
/// E.g. `type_lit! { key1: type1; key2: type2 }` will create a type literal with two members
/// named `key1` and `key2` with types `type1` and `type2` respectively.
macro_rules! type_lit {
    ($($k: ident: $t: path);* $(;)?) => {
        TsType::TypeLit(TsTypeLit {
            members: vec![$(
                TsTypeElement {
                    key: stringify!($k).to_string(),
                    type_ann: $t,
                    optional: false,
                    comments: vec![],
                }
            ),*],
        })
    };
}

impl TsType {
    /// Create a `TsType` from a stringified Rust identifier.
    pub fn from_name(
        config: &TypeGenerationConfig,
        path: &syn::Path,
        ident: &str,
        args: Vec<&syn::Type>,
        fn_output: Option<&syn::Type>,
        generics: &syn::Generics,
    ) -> Self {
        match ident {
            "u8" | "u16" | "u32" | "i8" | "i16" | "i32" | "f64" | "f32" => Self::NUMBER,

            "usize" | "isize" | "u64" | "i64" => {
                if cfg!(feature = "js") && config.large_number_types_as_bigints {
                    Self::BIGINT
                } else {
                    Self::NUMBER
                }
            }

            "u128" | "i128" => {
                if cfg!(feature = "js") {
                    Self::BIGINT
                } else {
                    Self::NUMBER
                }
            }

            "String" | "str" | "char" | "Path" | "PathBuf" => Self::STRING,

            "bool" => Self::BOOLEAN,

            "Box" | "Cow" | "Rc" | "Arc" | "Cell" | "RefCell" if args.len() == 1 => {
                Self::from_syn_type(config, args[0], generics)
            }

            "Vec" | "VecDeque" | "LinkedList" if args.len() == 1 => {
                let elem = Self::from_syn_type(config, args[0], generics);
                Self::Array(Box::new(elem))
            }

            "HashMap" | "BTreeMap" if args.len() == 2 => {
                let type_params = args
                    .iter()
                    .map(|arg| Self::from_syn_type(config, arg, generics))
                    .collect();

                let name = if cfg!(feature = "js") && !config.hashmap_as_object {
                    "Map"
                } else {
                    "Record"
                }
                .to_string();

                Self::Ref {
                    name,
                    source: super::TsTypeRefSource::Synthetic,
                    type_params,
                }
            }

            "HashSet" | "BTreeSet" if args.len() == 1 => {
                let elem = Self::from_syn_type(config, args[0], generics);
                Self::Array(Box::new(elem))
            }

            "Option" if args.len() == 1 => Self::Option(
                Box::new(Self::from_syn_type(config, args[0], generics)),
                NullType::new(config),
            ),

            "ByteBuf" => {
                if cfg!(feature = "js") {
                    Self::Ref {
                        name: String::from("Uint8Array"),
                        source: super::TsTypeRefSource::Synthetic,
                        type_params: vec![],
                    }
                } else {
                    Self::Array(Box::new(Self::NUMBER))
                }
            }

            "Result" if args.len() == 2 => {
                let arg0 = Self::from_syn_type(config, args[0], generics);
                let arg1 = Self::from_syn_type(config, args[1], generics);

                let ok = type_lit! { Ok: arg0 };
                let err = type_lit! { Err: arg1 };

                Self::Union(vec![ok, err])
            }

            "Duration" => type_lit! {
                secs: Self::NUMBER;
                nanos: Self::NUMBER;
            },

            "SystemTime" => type_lit! {
                secs_since_epoch: Self::NUMBER;
                nanos_since_epoch: Self::NUMBER;
            },

            // Treat as std::ops::Range or std::ops::RangeInclusive only when there is exactly one type parameter.
            // Otherwise, consider it a user-defined type and do not perform any conversion.
            "Range" | "RangeInclusive" if args.len() == 1 => {
                let start = Self::from_syn_type(config, args[0], generics);
                let end = start.clone();

                type_lit! {
                    start: start;
                    end: end;
                }
            }

            "Fn" | "FnOnce" | "FnMut" => {
                let params = args
                    .into_iter()
                    .map(|ty| Self::from_syn_type(config, ty, generics))
                    .collect();
                let type_ann = fn_output
                    .map(|ty| Self::from_syn_type(config, ty, generics))
                    .unwrap_or_else(|| TsType::VOID);

                Self::Fn {
                    params,
                    type_ann: Box::new(type_ann),
                }
            }
            _ => {
                let type_params = args
                    .into_iter()
                    .map(|ty| Self::from_syn_type(config, ty, generics))
                    .collect();
                let source = if generics
                    .type_params()
                    .any(|param| path.get_ident() == Some(&param.ident))
                {
                    super::TsTypeRefSource::TypeParam
                } else {
                    super::TsTypeRefSource::Rust(path.clone())
                };
                Self::Ref {
                    name: config.format_name(ident.to_string()),
                    source,
                    type_params,
                }
            }
        }
    }
}
