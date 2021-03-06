use std::collections::HashSet;

use crate::{DefDatabase, DefDatabaseStorage, ItemRes, Name};
use expect_test::expect;
use gqls_base_db::{InProject, SourceDatabaseStorage};
use gqls_fixture::fixture;
use maplit::{hashmap, hashset};
use smallvec::smallvec;
use testing::TestDatabaseExt;
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

#[test]
fn test_definitions() {
    let mut vfs = Vfs::default();

    let foo = vfs.intern("foo");
    let foogql = r#"
        type Foo @qux {
           bar: Bar @qux
        }

        type Foo {
            foo: Foo @d
        }

        type Bar {
            foo: Foo
        }

        extend type Bar implements Iface {
            i: Int! @qux
        }

        directive @qux on FIELD_DEFINITION | OBJECT | INPUT_OBJECT

        scalar S @qux

        union U @qux = Foo | Bar

        input I @qux {
            foo: Foo @qux
        }

        interface Iface @qux {
            foo: Foo @qux
        }
    "#;

    let bar = vfs.intern("bar");
    let bargql = r#"
        type Bar {
            baz: Baz
        }

        type Baz @foo {
            foo: Foo
        }

        directive @d on FIELD_DEFINITION
    "#;

    let fixture = fixture! {
        foo => foogql
        bar => bargql
    };

    let db = TestDB::from_fixture(&fixture);

    let item_map = db.item_map(foo);
    assert_eq!(
        *item_map,
        hashmap! {
            Name::unranged("Foo") => smallvec![idx!(0), idx!(1)],
            Name::unranged("Bar") => smallvec![idx!(2), idx!(3)],
            Name::unranged("@qux") => smallvec![idx!(4)],
            Name::unranged("S") => smallvec![idx!(5)],
            Name::unranged("U") => smallvec![idx!(6)],
            Name::unranged("I") => smallvec![idx!(7)],
            Name::unranged("Iface") => smallvec![idx!(8)],
        }
    );

    let resolutions = db.resolve_item(InProject::new(bar, Name::unranged("Foo")));
    assert_eq!(
        resolutions.into_item().as_slice(),
        [ItemRes::new(foo, idx!(0)), ItemRes::new(foo, idx!(1))]
    );

    let resolutions = db.resolve_item(InProject::new(foo, Name::unranged("Bar")));
    assert_eq!(
        resolutions.into_item().into_iter().collect::<HashSet<_>>(),
        hashset! {
            ItemRes::new(bar, idx!(0)),
            ItemRes::new(foo, idx!(2)),
            ItemRes::new(foo, idx!(3))
        }
    );

    let resolutions = db.resolve_item(InProject::new(bar, Name::unranged("@d")));
    assert_eq!(resolutions.into_item().as_slice(), [ItemRes::new(bar, idx!(2))]);

    let items = db.items(foo);
    expect![[r#"
        Items {
            items: Arena {
                len: 9,
                data: [
                    Item {
                        name: Foo,
                        range: 1:8..3:9,
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(0),
                        ),
                    },
                    Item {
                        name: Foo,
                        range: 5:8..7:9,
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(1),
                        ),
                    },
                    Item {
                        name: Bar,
                        range: 9:8..11:9,
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(2),
                        ),
                    },
                    Item {
                        name: Bar,
                        range: 13:8..15:9,
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(3),
                        ),
                    },
                    Item {
                        name: @qux,
                        range: 17:8..17:66,
                        kind: DirectiveDefinition(
                            Idx::<DirectiveDefinition>(0),
                        ),
                    },
                    Item {
                        name: S,
                        range: 19:8..19:21,
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(4),
                        ),
                    },
                    Item {
                        name: U,
                        range: 21:8..21:32,
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(5),
                        ),
                    },
                    Item {
                        name: I,
                        range: 23:8..25:9,
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(6),
                        ),
                    },
                    Item {
                        name: Iface,
                        range: 27:8..29:9,
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(7),
                        ),
                    },
                ],
            },
            typedefs: Arena {
                len: 8,
                data: [
                    TypeDefinition {
                        directives: [
                            @qux,
                        ],
                        implementations: None,
                        kind: Object,
                        is_ext: false,
                    },
                    TypeDefinition {
                        directives: [],
                        implementations: None,
                        kind: Object,
                        is_ext: false,
                    },
                    TypeDefinition {
                        directives: [],
                        implementations: None,
                        kind: Object,
                        is_ext: false,
                    },
                    TypeDefinition {
                        directives: [],
                        implementations: Some(
                            {
                                Iface,
                            },
                        ),
                        kind: Object,
                        is_ext: true,
                    },
                    TypeDefinition {
                        directives: [
                            @qux,
                        ],
                        implementations: None,
                        kind: Scalar,
                        is_ext: false,
                    },
                    TypeDefinition {
                        directives: [
                            @qux,
                        ],
                        implementations: None,
                        kind: Union,
                        is_ext: false,
                    },
                    TypeDefinition {
                        directives: [
                            @qux,
                        ],
                        implementations: None,
                        kind: Input,
                        is_ext: false,
                    },
                    TypeDefinition {
                        directives: [
                            @qux,
                        ],
                        implementations: None,
                        kind: Interface,
                        is_ext: false,
                    },
                ],
            },
            directives: Arena {
                len: 1,
                data: [
                    DirectiveDefinition {
                        locations: FIELD_DEFINITION | INPUT_OBJECT | OBJECT,
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
                        name: Bar,
                        range: 1:8..3:9,
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(0),
                        ),
                    },
                    Item {
                        name: Baz,
                        range: 5:8..7:9,
                        kind: TypeDefinition(
                            Idx::<TypeDefinition>(1),
                        ),
                    },
                    Item {
                        name: @d,
                        range: 9:8..9:40,
                        kind: DirectiveDefinition(
                            Idx::<DirectiveDefinition>(0),
                        ),
                    },
                ],
            },
            typedefs: Arena {
                len: 2,
                data: [
                    TypeDefinition {
                        directives: [],
                        implementations: None,
                        kind: Object,
                        is_ext: false,
                    },
                    TypeDefinition {
                        directives: [
                            @foo,
                        ],
                        implementations: None,
                        kind: Object,
                        is_ext: false,
                    },
                ],
            },
            directives: Arena {
                len: 1,
                data: [
                    DirectiveDefinition {
                        locations: FIELD_DEFINITION,
                    },
                ],
            },
        }
    "#]]
    .assert_debug_eq(&items);
}
