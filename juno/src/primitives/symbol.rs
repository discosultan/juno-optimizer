use std::iter::once;

pub trait SymbolExt {
    fn assets(&self) -> (&str, &str);
    fn base_asset(&self) -> &str;
    fn quote_asset(&self) -> &str;
}

impl SymbolExt for str {
    fn assets(&self) -> (&str, &str) {
        let dash_i = dash_index(self);
        (&self[..dash_i], &self[dash_i + 1..])
    }
    fn base_asset(&self) -> &str {
        &self[..dash_index(self)]
    }
    fn quote_asset(&self) -> &str {
        &self[dash_index(self) + 1..]
    }
}

pub fn list_assets(symbols: &[String]) -> Vec<&str> {
    symbols
        .iter()
        .map(|symbol| symbol.assets())
        .flat_map(|(base, quote)| once(base).chain(once(quote)))
        .collect()
}

fn dash_index(value: &str) -> usize {
    value.find('-').expect("not a valid symbol")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_assets() {
        assert_eq!("eth-btc".assets(), ("eth", "btc"));
    }

    #[test]
    fn test_symbol_base_asset() {
        assert_eq!("eth-btc".base_asset(), "eth");
    }

    #[test]
    fn test_symbol_quote_asset() {
        assert_eq!("eth-btc".quote_asset(), "btc");
    }
}
