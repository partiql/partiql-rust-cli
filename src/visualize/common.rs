pub(crate) trait ToDotGraph<T> {
    fn to_graph(self, data: &T) -> String;
}

pub(crate) const BG_COLOR: &'static str = "\"#002b3600\"";
pub(crate) const FG_COLOR: &'static str = "\"#839496\"";
