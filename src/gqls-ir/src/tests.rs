use std::path::Path;

use crate::{DefDatabase, DefDatabaseStorage, Name, Res};
use expect_test::expect;
use gqls_base_db::SourceDatabaseStorage;
use maplit::hashmap;
use smallvec::smallvec;
use vfs::Vfs;

#[salsa::database(SourceDatabaseStorage, DefDatabaseStorage)]
#[derive(Default)]
pub(crate) struct TestDB {
    storage: salsa::Storage<TestDB>,
}

impl salsa::Database for TestDB {
}

macro_rules! idx {
    ($idx:expr) => {
        la_arena::Idx::from_raw(la_arena::RawIdx::from($idx))
    };
}

pub(crate) use idx;

macro_rules! setup {
    ($db:ident: {
        $($file:ident: $text:expr,)*
     }) => {
        use gqls_base_db::{FileData, SourceDatabase};
        $db.set_projects(std::sync::Arc::new(
            maplit::hashmap! { "default" => maplit::hashset! { $($file),* } },
        ));
        $(
            $db.set_file_data($file, FileData::new($text, gqls_parse::parse_fresh($text)));
        )*
    };
}

pub(crate) use setup;

#[test]
fn test_definitions() {
    let mut db = TestDB::default();
    let mut vfs = Vfs::default();

    let foo = vfs.intern("foo");
    let foogql = r#"
        type Foo {
           bar: Bar
        }

        type Foo {
            foo: Foo
        }

        type Bar {
            foo: Foo
        }

        extend type Bar {
            i: Int!
        }
    "#;

    let bar = vfs.intern("bar");
    let bargql = r#"
        type Bar {
            baz: Baz
        }

        type Baz {
            foo: Foo
        }

        directive @d on FIELD
    "#;

    setup!(db: {
        foo: foogql,
        bar: bargql,
    });

    let item_map = db.item_map(foo);
    assert_eq!(
        *item_map,
        hashmap! {
            Name::new("Foo") => smallvec![idx!(0), idx!(1)],
            Name::new("Bar") => smallvec![idx!(2), idx!(3)],
        }
    );

    let resolutions = db.resolve(bar, Name::new("Foo"));
    assert_eq!(
        resolutions.as_slice(),
        [Res { file: foo, idx: idx!(0) }, Res { file: foo, idx: idx!(1) }]
    );

    let mut resolutions = db.resolve(foo, Name::new("Bar"));
    resolutions.sort();
    assert_eq!(
        resolutions.as_slice(),
        [
            Res { file: bar, idx: idx!(0) },
            Res { file: foo, idx: idx!(2) },
            Res { file: foo, idx: idx!(3) },
        ]
    );

    let resolutions = db.resolve(bar, Name::new("d"));
    assert_eq!(resolutions.as_slice(), [Res { file: Path::new("bar"), idx: idx!(2) },]);

    let items = db.items(foo);
    expect![[r#"
        Items {
            items: Arena {
                len: 4,
                data: [
                    Item {
                        range: Range {
                            start_byte: 9,
                            end_byte: 49,
                            start_point: Point {
                                row: 1,
                                column: 8,
                            },
                            end_point: Point {
                                row: 3,
                                column: 9,
                            },
                        },
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(0),
                        ),
                    },
                    Item {
                        range: Range {
                            start_byte: 59,
                            end_byte: 100,
                            start_point: Point {
                                row: 5,
                                column: 8,
                            },
                            end_point: Point {
                                row: 7,
                                column: 9,
                            },
                        },
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(1),
                        ),
                    },
                    Item {
                        range: Range {
                            start_byte: 110,
                            end_byte: 151,
                            start_point: Point {
                                row: 9,
                                column: 8,
                            },
                            end_point: Point {
                                row: 11,
                                column: 9,
                            },
                        },
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(2),
                        ),
                    },
                    Item {
                        range: Range {
                            start_byte: 161,
                            end_byte: 208,
                            start_point: Point {
                                row: 13,
                                column: 8,
                            },
                            end_point: Point {
                                row: 15,
                                column: 9,
                            },
                        },
                        kind: TypeExtension(
                            Idx::<TypeExtension>(0),
                        ),
                    },
                ],
            },
            types: Arena {
                len: 3,
                data: [
                    TypeDefinition {
                        name: Foo,
                    },
                    TypeDefinition {
                        name: Foo,
                    },
                    TypeDefinition {
                        name: Bar,
                    },
                ],
            },
            directives: Arena {
                len: 0,
                data: [],
            },
            type_exts: Arena {
                len: 1,
                data: [
                    TypeExtension {
                        name: Bar,
                    },
                ],
            },
        }
    "#]]
    .assert_debug_eq(&items);

    let items = db.items(bar);
    expect![[r#"
        Items {
            items: Arena {
                len: 3,
                data: [
                    Item {
                        range: Range {
                            start_byte: 9,
                            end_byte: 50,
                            start_point: Point {
                                row: 1,
                                column: 8,
                            },
                            end_point: Point {
                                row: 3,
                                column: 9,
                            },
                        },
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(0),
                        ),
                    },
                    Item {
                        range: Range {
                            start_byte: 60,
                            end_byte: 101,
                            start_point: Point {
                                row: 5,
                                column: 8,
                            },
                            end_point: Point {
                                row: 7,
                                column: 9,
                            },
                        },
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(1),
                        ),
                    },
                    Item {
                        range: Range {
                            start_byte: 111,
                            end_byte: 132,
                            start_point: Point {
                                row: 9,
                                column: 8,
                            },
                            end_point: Point {
                                row: 9,
                                column: 29,
                            },
                        },
                        kind: DirectiveDefinition(
                            Idx::<DirectiveDefinition>(0),
                        ),
                    },
                ],
            },
            types: Arena {
                len: 2,
                data: [
                    TypeDefinition {
                        name: Bar,
                    },
                    TypeDefinition {
                        name: Baz,
                    },
                ],
            },
            directives: Arena {
                len: 1,
                data: [
                    DirectiveDefinition {
                        name: d,
                    },
                ],
            },
            type_exts: Arena {
                len: 0,
                data: [],
            },
        }
    "#]]
    .assert_debug_eq(&items);
}
