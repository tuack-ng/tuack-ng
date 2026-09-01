use std::marker::PhantomData;

use indexmap::IndexMap;
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::prelude::*;

/// 使 `IndexMap` 可以跨越容器使用 `serde_many` 的视图标记。
///
/// `serde_many` 无法直接为 `IndexMap` 实现 `SerializeMany` / `DeserializeMany`
/// （孤儿规则 + 与 blanket impl 的相干性冲突）。此包装器把 `IndexMap` 包在本地
/// 类型中，并为任意标记 `M` 实现 `SerializeMany<M>` / `DeserializeMany<'de, M>`，
/// 元素按 `V: SerializeMany<M>` 序列化，从而跨过标准库容器的限制。
#[derive(Debug, Clone)]
pub struct IndexMapMany<K, V>(IndexMap<K, V>);

impl<K, V> IndexMapMany<K, V> {
    /// 用已有的 `IndexMap` 构造包装器
    pub fn new(map: IndexMap<K, V>) -> Self {
        Self(map)
    }

    /// 解包，返回内部的 `IndexMap`
    pub fn into_inner(self) -> IndexMap<K, V> {
        self.0
    }

    /// 获取内部 `IndexMap` 的引用
    pub fn inner(&self) -> &IndexMap<K, V> {
        &self.0
    }

    /// 获取内部 `IndexMap` 的可变引用
    pub fn inner_mut(&mut self) -> &mut IndexMap<K, V> {
        &mut self.0
    }
}

impl<K, V> From<IndexMap<K, V>> for IndexMapMany<K, V> {
    fn from(map: IndexMap<K, V>) -> Self {
        Self(map)
    }
}

impl<K, V> From<IndexMapMany<K, V>> for IndexMap<K, V> {
    fn from(map: IndexMapMany<K, V>) -> Self {
        map.0
    }
}

impl<K, V> Default for IndexMapMany<K, V> {
    fn default() -> Self {
        Self(IndexMap::new())
    }
}

impl<K, V> IntoIterator for IndexMapMany<K, V> {
    type Item = (K, V);
    type IntoIter = indexmap::map::IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, K, V> IntoIterator for &'a IndexMapMany<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = indexmap::map::Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a, K, V> IntoIterator for &'a mut IndexMapMany<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = indexmap::map::IterMut<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl<K, V> std::ops::Deref for IndexMapMany<K, V> {
    type Target = IndexMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K, V> std::ops::DerefMut for IndexMapMany<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<K, V, M> SerializeMany<M> for IndexMapMany<K, V>
where
    K: Serialize,
    V: SerializeMany<M>,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k, &ManyValueRef::<M, V>(v, PhantomData))?;
        }
        map.end()
    }
}

/// 借用一个 `&V`，按标记 `M` 序列化（实现 serde `Serialize` 以便塞进容器）。
struct ManyValueRef<'a, M, V>(&'a V, PhantomData<M>);

impl<'a, M, V> Serialize for ManyValueRef<'a, M, V>
where
    V: SerializeMany<M>,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SerializeMany::<M>::serialize(self.0, serializer)
    }
}

impl<'de, K, V, M> DeserializeMany<'de, M> for IndexMapMany<K, V>
where
    K: Deserialize<'de> + std::hash::Hash + Eq,
    V: DeserializeMany<'de, M>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(IndexMapManyVisitor::<K, V, M>(PhantomData))
    }
}

struct IndexMapManyVisitor<K, V, M>(PhantomData<(K, V, M)>);

impl<'de, K, V, M> Visitor<'de> for IndexMapManyVisitor<K, V, M>
where
    K: Deserialize<'de> + std::hash::Hash + Eq,
    V: DeserializeMany<'de, M>,
{
    type Value = IndexMapMany<K, V>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a map")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
        let mut map = IndexMap::with_capacity(access.size_hint().unwrap_or(0));
        while let Some(key) = access.next_key::<K>()? {
            let value = access.next_value_seed(ManyValueSeed::<V, M>(PhantomData))?;
            map.insert(key, value);
        }
        Ok(IndexMapMany(map))
    }
}

struct ManyValueSeed<V, M>(PhantomData<(V, M)>);

impl<'de, V, M> serde::de::DeserializeSeed<'de> for ManyValueSeed<V, M>
where
    V: DeserializeMany<'de, M>,
{
    type Value = V;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<V, D::Error> {
        DeserializeMany::<M>::deserialize(deserializer)
    }
}
