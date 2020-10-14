pub mod emoji_data;
pub mod guild_data;

use crate::MaybeError;
use std::{
    cmp::Eq,
    hash::Hash,
    marker::{Send, Sync},
};

pub trait ClientData {
    // As needed by serenity::prelude::ShareMap and HashMap's K
    type Id: 'static + Sync + Send + Eq + Hash + Copy;
    fn from_file(id: Self::Id) -> Self;
    fn save(&mut self) -> MaybeError;
}
