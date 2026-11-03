//! Deep merge of TOML tables.

/// Merges `overlay` into `base`: nested tables merge key by key, any other
/// value (including arrays) replaces the one in `base`.
pub(crate) fn merge_into(base: &mut toml::Table, overlay: &toml::Table) {
    for (key, value) in overlay {
        match (base.get_mut(key), value) {
            (Some(toml::Value::Table(base_table)), toml::Value::Table(overlay_table)) => {
                merge_into(base_table, overlay_table);
            }
            _ => {
                base.insert(key.clone(), value.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> toml::Table {
        text.parse().unwrap()
    }

    #[test]
    fn nested_tables_merge_and_scalars_replace() {
        let mut base = table("a = 1\nb = [1, 2]\n[nested]\nx = 1\ny = 2");
        merge_into(&mut base, &table("b = [3]\n[nested]\ny = 20\nz = 30"));

        assert_eq!(
            base,
            table("a = 1\nb = [3]\n[nested]\nx = 1\ny = 20\nz = 30")
        );
    }
}
