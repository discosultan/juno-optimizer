use syn::Field;

pub fn is_chromosome(field: &Field) -> bool {
    field
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("chromosome"))
}

pub fn is_serde_default(field: &Field) -> bool {
    field.attrs.iter().any(|attr| {
        if attr.path().is_ident("serde") {
            let mut is_serde_default = false;
            let parse_result = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("default") {
                    is_serde_default = true;
                    return Ok(());
                }
                Ok(())
            });
            if parse_result.is_err() {
                return false;
            };
            return is_serde_default;
        }
        false
    })
}
