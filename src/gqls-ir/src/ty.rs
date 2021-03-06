use crate::{ItemRes, Name, Range, Res};
use std::fmt::{self, Debug};
use std::sync::Arc;

pub type Ty = Arc<Type>;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Type {
    pub range: Range,
    pub kind: TyKind,
}

impl Type {
    pub fn item_resolutions(&self) -> &[ItemRes] {
        match &self.kind {
            TyKind::Named(_, res) => match res {
                Res::Item(res) => res,
                _ => &[],
            },
            TyKind::NonNull(ty) | TyKind::List(ty) => ty.item_resolutions(),
            TyKind::Err(_) => &[],
        }
    }

    pub fn name(&self) -> Name {
        match &self.kind {
            TyKind::Named(name, _) | TyKind::Err(name) => name.clone(),
            TyKind::NonNull(ty) | TyKind::List(ty) => ty.name(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum TyKind {
    Named(Name, Res),
    NonNull(Ty),
    List(Ty),
    Err(Name),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scalar {
    Id,
    Int,
    String,
    Float,
    Custom(CustomScalar),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomScalar {}

impl Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.kind, f)
    }
}

impl Debug for TyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            TyKind::Named(name, _) | TyKind::Err(name) => write!(f, "{}", name.name),
            TyKind::NonNull(ty) => write!(f, "{ty:?}!"),
            TyKind::List(ty) => write!(f, "[{ty:?}]"),
        }
    }
}
