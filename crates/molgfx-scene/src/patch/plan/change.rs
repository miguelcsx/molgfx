//! A value a patch leaves alone or sets.

#[derive(Clone, Default)]
pub(super) enum Change<T> {
    #[default]
    Unchanged,
    Set(Option<T>),
}

impl<T> Change<T> {
    pub(super) fn value(&self) -> Option<&T> {
        match self {
            Self::Set(Some(value)) => Some(value),
            Self::Unchanged | Self::Set(None) => None,
        }
    }
}

pub(super) fn assign<T>(target: &mut Option<T>, update: Change<T>) {
    if let Change::Set(update) = update {
        *target = update;
    }
}
