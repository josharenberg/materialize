// Copyright Materialize, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::BTreeSet;

use mz_proto::{IntoRustIfSome, RustType, TryFromProtoError};
use mz_repr::ColumnType;
use proptest::prelude::any;
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/mz_mysql_util.rs"));

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Arbitrary)]
pub struct MySqlTableDesc {
    /// In MySQL the schema and database of a table are synonymous.
    pub schema_name: String,
    /// The name of the table.
    pub name: String,
    /// Columns for the table
    ///
    /// The index of each column is based on its `ordinal_position`
    /// reported by the information_schema.columns table, which defines
    /// the order of column values when received in a row.
    #[proptest(strategy = "proptest::collection::vec(any::<MySqlColumnDesc>(), 0..4)")]
    pub columns: Vec<MySqlColumnDesc>,
    /// Applicable keys for this table (i.e. primary key and unique
    /// constraints).
    #[proptest(strategy = "proptest::collection::btree_set(any::<MySqlKeyDesc>(), 0..4)")]
    pub keys: BTreeSet<MySqlKeyDesc>,
}

impl RustType<ProtoMySqlTableDesc> for MySqlTableDesc {
    fn into_proto(&self) -> ProtoMySqlTableDesc {
        ProtoMySqlTableDesc {
            schema_name: self.schema_name.clone(),
            name: self.name.clone(),
            columns: self.columns.iter().map(|c| c.into_proto()).collect(),
            keys: self.keys.iter().map(|c| c.into_proto()).collect(),
        }
    }

    fn from_proto(proto: ProtoMySqlTableDesc) -> Result<Self, TryFromProtoError> {
        Ok(Self {
            schema_name: proto.schema_name,
            name: proto.name,
            columns: proto
                .columns
                .into_iter()
                .map(MySqlColumnDesc::from_proto)
                .collect::<Result<_, _>>()?,
            keys: proto
                .keys
                .into_iter()
                .map(MySqlKeyDesc::from_proto)
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Arbitrary)]
pub struct MySqlColumnDesc {
    /// The name of the column.
    pub name: String,
    /// The MySQL datatype of the column.
    pub column_type: ColumnType,
}

impl RustType<ProtoMySqlColumnDesc> for MySqlColumnDesc {
    fn into_proto(&self) -> ProtoMySqlColumnDesc {
        ProtoMySqlColumnDesc {
            name: self.name.clone(),
            column_type: Some(self.column_type.into_proto()),
        }
    }

    fn from_proto(proto: ProtoMySqlColumnDesc) -> Result<Self, TryFromProtoError> {
        Ok(Self {
            name: proto.name,
            column_type: proto
                .column_type
                .into_rust_if_some("ProtoMySqlColumnDesc::column_type")?,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd, Arbitrary)]
pub struct MySqlKeyDesc {
    /// The name of the index.
    pub name: String,
    /// Whether or not this key is the primary key.
    pub is_primary: bool,
    /// The columns that make up the key.
    #[proptest(strategy = "proptest::collection::vec(any::<String>(), 0..4)")]
    pub columns: Vec<String>,
}

impl RustType<ProtoMySqlKeyDesc> for MySqlKeyDesc {
    fn into_proto(&self) -> ProtoMySqlKeyDesc {
        ProtoMySqlKeyDesc {
            name: self.name.clone(),
            is_primary: self.is_primary.clone(),
            columns: self.columns.clone(),
        }
    }

    fn from_proto(proto: ProtoMySqlKeyDesc) -> Result<Self, TryFromProtoError> {
        Ok(Self {
            name: proto.name,
            is_primary: proto.is_primary,
            columns: proto.columns,
        })
    }
}
