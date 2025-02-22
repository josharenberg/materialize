// Copyright Materialize, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Types for describing dataflow sinks.

use mz_expr::refresh_schedule::RefreshSchedule;
use mz_proto::{IntoRustIfSome, ProtoType, RustType, TryFromProtoError};
use mz_repr::{GlobalId, RelationDesc, Timestamp};
use mz_storage_types::controller::CollectionMetadata;
use proptest::prelude::{any, Arbitrary, BoxedStrategy, Strategy};
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use timely::progress::Antichain;

include!(concat!(env!("OUT_DIR"), "/mz_compute_types.sinks.rs"));

/// A sink for updates to a relational collection.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ComputeSinkDesc<S: 'static = (), T = Timestamp> {
    /// TODO(#25239): Add documentation.
    pub from: GlobalId,
    /// TODO(#25239): Add documentation.
    pub from_desc: RelationDesc,
    /// TODO(#25239): Add documentation.
    pub connection: ComputeSinkConnection<S>,
    /// TODO(#25239): Add documentation.
    pub with_snapshot: bool,
    /// TODO(#25239): Add documentation.
    pub up_to: Antichain<T>,
    /// TODO(#25239): Add documentation.
    pub non_null_assertions: Vec<usize>,
    /// TODO(#25239): Add documentation.
    pub refresh_schedule: Option<RefreshSchedule>,
}

impl Arbitrary for ComputeSinkDesc<CollectionMetadata, Timestamp> {
    type Strategy = BoxedStrategy<Self>;
    type Parameters = ();

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (
            any::<GlobalId>(),
            any::<RelationDesc>(),
            any::<ComputeSinkConnection<CollectionMetadata>>(),
            any::<bool>(),
            proptest::collection::vec(any::<Timestamp>(), 1..4),
            proptest::collection::vec(any::<usize>(), 0..4),
            proptest::option::of(any::<RefreshSchedule>()),
        )
            .prop_map(
                |(
                    from,
                    from_desc,
                    connection,
                    with_snapshot,
                    up_to_frontier,
                    non_null_assertions,
                    refresh_schedule,
                )| {
                    ComputeSinkDesc {
                        from,
                        from_desc,
                        connection,
                        with_snapshot,
                        up_to: Antichain::from(up_to_frontier),
                        non_null_assertions,
                        refresh_schedule,
                    }
                },
            )
            .boxed()
    }
}

impl RustType<ProtoComputeSinkDesc> for ComputeSinkDesc<CollectionMetadata, Timestamp> {
    fn into_proto(&self) -> ProtoComputeSinkDesc {
        ProtoComputeSinkDesc {
            connection: Some(self.connection.into_proto()),
            from: Some(self.from.into_proto()),
            from_desc: Some(self.from_desc.into_proto()),
            with_snapshot: self.with_snapshot,
            up_to: Some(self.up_to.into_proto()),
            non_null_assertions: self.non_null_assertions.into_proto(),
            refresh_schedule: self.refresh_schedule.into_proto(),
        }
    }

    fn from_proto(proto: ProtoComputeSinkDesc) -> Result<Self, TryFromProtoError> {
        Ok(ComputeSinkDesc {
            from: proto.from.into_rust_if_some("ProtoComputeSinkDesc::from")?,
            from_desc: proto
                .from_desc
                .into_rust_if_some("ProtoComputeSinkDesc::from_desc")?,
            connection: proto
                .connection
                .into_rust_if_some("ProtoComputeSinkDesc::connection")?,
            with_snapshot: proto.with_snapshot,
            up_to: proto
                .up_to
                .into_rust_if_some("ProtoComputeSinkDesc::up_to")?,
            non_null_assertions: proto.non_null_assertions.into_rust()?,
            refresh_schedule: proto.refresh_schedule.into_rust()?,
        })
    }
}

/// TODO(#25239): Add documentation.
#[derive(Arbitrary, Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum ComputeSinkConnection<S: 'static = ()> {
    /// TODO(#25239): Add documentation.
    Subscribe(SubscribeSinkConnection),
    /// TODO(#25239): Add documentation.
    Persist(PersistSinkConnection<S>),
    /// TODO(#25239): Add documentation.
    S3Oneshot(S3OneshotSinkConnection),
}

impl<S> ComputeSinkConnection<S> {
    /// Returns the name of the sink connection.
    pub fn name(&self) -> &'static str {
        match self {
            ComputeSinkConnection::Subscribe(_) => "subscribe",
            ComputeSinkConnection::Persist(_) => "persist",
            ComputeSinkConnection::S3Oneshot(_) => "s3",
        }
    }

    /// True if the sink is a subscribe, which is differently recoverable than other sinks.
    pub fn is_subscribe(&self) -> bool {
        if let ComputeSinkConnection::Subscribe(_) = self {
            true
        } else {
            false
        }
    }
}

impl RustType<ProtoComputeSinkConnection> for ComputeSinkConnection<CollectionMetadata> {
    fn into_proto(&self) -> ProtoComputeSinkConnection {
        use proto_compute_sink_connection::Kind;
        ProtoComputeSinkConnection {
            kind: Some(match self {
                ComputeSinkConnection::Subscribe(_) => Kind::Subscribe(()),
                ComputeSinkConnection::Persist(persist) => Kind::Persist(persist.into_proto()),
                ComputeSinkConnection::S3Oneshot(s3) => Kind::S3Oneshot(s3.into_proto()),
            }),
        }
    }

    fn from_proto(proto: ProtoComputeSinkConnection) -> Result<Self, TryFromProtoError> {
        use proto_compute_sink_connection::Kind;
        let kind = proto
            .kind
            .ok_or_else(|| TryFromProtoError::missing_field("ProtoComputeSinkConnection::kind"))?;
        Ok(match kind {
            Kind::Subscribe(_) => ComputeSinkConnection::Subscribe(SubscribeSinkConnection {}),
            Kind::Persist(persist) => ComputeSinkConnection::Persist(persist.into_rust()?),
            Kind::S3Oneshot(s3) => ComputeSinkConnection::S3Oneshot(s3.into_rust()?),
        })
    }
}

/// TODO(#25239): Add documentation.
#[derive(Arbitrary, Default, Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SubscribeSinkConnection {}

/// TODO(#25239): Add documentation.
#[derive(Arbitrary, Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct S3OneshotSinkConnection {
    /// TODO(#25239): Add documentation.
    pub prefix: String,
    /// TODO(#25239): Add documentation.
    pub aws_connection: mz_storage_types::connections::aws::AwsConnection,
}

impl RustType<ProtoS3OneshotSinkConnection> for S3OneshotSinkConnection {
    fn into_proto(&self) -> ProtoS3OneshotSinkConnection {
        ProtoS3OneshotSinkConnection {
            prefix: self.prefix.clone(),
            aws_connection: Some(self.aws_connection.into_proto()),
        }
    }

    fn from_proto(proto: ProtoS3OneshotSinkConnection) -> Result<Self, TryFromProtoError> {
        Ok(S3OneshotSinkConnection {
            prefix: proto.prefix,
            aws_connection: proto
                .aws_connection
                .into_rust_if_some("ProtoS3OneshotSinkConnection::aws_connection")?,
        })
    }
}

/// TODO(#25239): Add documentation.
#[derive(Arbitrary, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistSinkConnection<S> {
    /// TODO(#25239): Add documentation.
    pub value_desc: RelationDesc,
    /// TODO(#25239): Add documentation.
    pub storage_metadata: S,
}

impl RustType<ProtoPersistSinkConnection> for PersistSinkConnection<CollectionMetadata> {
    fn into_proto(&self) -> ProtoPersistSinkConnection {
        ProtoPersistSinkConnection {
            value_desc: Some(self.value_desc.into_proto()),
            storage_metadata: Some(self.storage_metadata.into_proto()),
        }
    }

    fn from_proto(proto: ProtoPersistSinkConnection) -> Result<Self, TryFromProtoError> {
        Ok(PersistSinkConnection {
            value_desc: proto
                .value_desc
                .into_rust_if_some("ProtoPersistSinkConnection::value_desc")?,
            storage_metadata: proto
                .storage_metadata
                .into_rust_if_some("ProtoPersistSinkConnection::storage_metadata")?,
        })
    }
}
