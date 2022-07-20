use syn::{Field, Meta, NestedMeta};

pub fn is_chromosome(field: &Field) -> bool {
    field
        .attrs
        .iter()
        .any(|attr| attr.path.is_ident("chromosome"))
}

pub fn is_serde_default(field: &Field) -> bool {
    field.attrs.iter().any(|attr| {
        if attr.path.is_ident("serde") {
            let meta = attr.parse_meta().unwrap();
            if let Meta::List(meta) = meta {
                if let NestedMeta::Meta(Meta::Path(path)) = &meta.nested[0] {
                    if path.is_ident("default") {
                        return true;
                    }
                }
            }
        }
        false
    })
}
