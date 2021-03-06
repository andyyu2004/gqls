#[macro_export]
macro_rules! position {
    ($file:ident:$row:literal:$col:expr) => {
        $crate::Position { file: $file, point: point!($row: $col) }
    };
}

#[macro_export]
macro_rules! point {
    ($row:literal:$col:expr) => {
        $crate::tree_sitter::Point { row: $row, column: $col }
    };
}

#[macro_export]
macro_rules! range {
    ($a:literal:$b:literal..$x:literal:$y:literal) => {
        $crate::Range { start: $crate::point!($a: $b), end: $crate::point!($x: $y) }
    };
}
